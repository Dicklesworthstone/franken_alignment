//! Real actor, helper and reviewer sockets; synthetic verdicts are not model evidence.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{ReviewClientProgress};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use fa_reference::action::consequence::oversight::actor_wire::{Command, WireResponse, decode_response, encode_command};
use fa_reference::action::consequence::oversight::actor_wire::client::{ActorExchange, ClientIoBudget, ClientIoLimits, ClientProgress};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-live-{}-{stamp}-{n}", std::process::id()));
        fs::create_dir(&path).unwrap();
        for name in ["actors", "reviewers"] { fs::DirBuilder::new().mode(0o750).create(path.join(name)).unwrap(); }
        Self(path)
    }
}
impl Drop for Directory { fn drop(&mut self) { if let Err(e) = fs::remove_dir_all(&self.0) { eprintln!("live fixture cleanup: {e}"); } } }
fn configured(root: &Directory) -> Config {
    let mut c = Config::decode(include_bytes!("../../../../fixtures/supervised_publication.json")).unwrap();
    c.store = root.0.join("store");
    c.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51, c.profile.delivery.scope, 1048576).unwrap();
    c.timing.poll_ms = 1; c.timing.runtime_ms = 15000; c.timing.cleanup_ms = 2000;
    c.programs = ["alpha", "beta"].into_iter().map(|member| (member.to_owned(), HelperProgram::new(
        std::env::current_exe().unwrap(), root.0.clone(),
        vec!["--exact".into(), "tests::synthetic_helper_process".into(), "--nocapture".into()],
        BTreeMap::from([(OsString::from("FA_EXAMPLE_HELPER_MEMBER"), OsString::from(member))])).unwrap())).collect();
    c
}
fn credentials() -> PeerCredentials { let (socket, _) = UnixStream::pair().unwrap(); PeerCredentials::observe(&socket).unwrap() }
fn scope(c: &Config) -> String {
    let s = c.profile.delivery.scope;
    format!(r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#, s.tenant,s.principal,s.run,s.branch,s.authority)
}
fn actor_json(root: &Directory, c: &Config) -> String {
    let id = credentials();
    format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":7,"scope":{},"socket":"{}/actors/actor.sock","supervisor":{{"uid":{},"gid":{},"pid":{}}},"actor":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":8,"connection_limit":4,"exchange_limit":1024,"runtime_ms":15000,"poll_ms":1,"reply_ms":200}}"#,
        scope(c), root.0.display(), id.uid(),id.gid(),id.pid(),id.uid(),id.gid(),id.pid())
}
fn profiles(root: &Directory, c: &Config) -> (Profile, PeerProfile) {
    let id=credentials();
    let peers=format!(r#"{{"version":1,"clock":"unix_milliseconds","scope":{},"reviewer_id":{},"socket_directory":"{}/reviewers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":8,"runtime_ms":15000,"poll_ms":1}}"#,
        scope(c),c.profile.human.reviewer_id,root.0.display(),id.uid(),id.gid(),id.pid(),id.uid(),id.gid(),id.pid());
    (Profile::decode(actor_json(root,c).as_bytes()).unwrap(),PeerProfile::decode(peers.as_bytes()).unwrap())
}
fn evidence(root: &Directory,c: &Config) {
    let value=EvidenceSnapshot::new(EvidenceIdentity { source:51,generation:1,scope:c.profile.delivery.scope },
        Snapshot { semantic_epoch:1,complete:true,values:BTreeMap::from([(7,b"ok".to_vec())]) },
        ["alpha","beta"].into_iter().map(|m|(m.to_owned(),format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    fs::write(root.0.join("evidence.json"),value.encode()).unwrap();
}
fn proposal(c: &Config)->Command { Command::Submit { request:7,proposal:ActorProposal {
    target:c.profile.delivery.target,payload:b"network".to_vec(),units:7,deadline:ElapsedTick(100000),expected_policy_epoch:0 } } }
fn wait(path: &Path) {
    let started=Instant::now(); while !path.exists() { assert!(started.elapsed()<Duration::from_secs(10),"missing {path:?}"); pause(1); }
}
fn connect(p: &Profile)->UnixStream {
    wait(&p.socket); let stream=UnixStream::connect(&p.socket).unwrap(); p.supervisor.verify(&stream).unwrap();
    stream.set_nonblocking(true).unwrap(); stream
}
fn exchange(stream:UnixStream,cmd:Command,budget:&mut ClientIoBudget)->(UnixStream,WireResponse) {
    let mut x=ActorExchange::new(stream,cmd,budget).unwrap(); let start=Instant::now();
    loop { assert!(start.elapsed()<Duration::from_secs(10)); match x.step(budget).unwrap() {
        ClientProgress::Response(response)=>return(x.into_stream().unwrap(),response),
        ClientProgress::Complete=>panic!("missing response"), _=>pause(1),
    } }
}
fn human(profile: PeerProfile)->std::thread::JoinHandle<()> {
    std::thread::spawn(move||{
        wait(&profile.socket(7)); let mut client=profile.connect_client(7).unwrap(); let start=Instant::now();
        loop { assert!(start.elapsed()<Duration::from_secs(10)); match client.step().unwrap() {
            ReviewClientProgress::NeedsDecision=>client.respond(ReviewDecision::Approve).unwrap(),
            ReviewClientProgress::Complete=>return, _=>pause(1),
        } }
    })
}
fn actor_submit(profile:Profile,path:PathBuf)->std::thread::JoinHandle<WireResponse> {
    std::thread::spawn(move||{ wait(&profile.socket); let mut out=Vec::new(); client::submit(&profile,&path,&mut out).unwrap();
        decode_response(out.strip_suffix(b"\n").unwrap()).unwrap() })
}

#[test]
fn live_actor_runs_both_keys_and_native_publication_without_operator_submit_file() {
    let root=Directory::new();let c=configured(&root); evidence(&root,&c);let (actor,reviewer)=profiles(&root,&c);
    let path=root.0.join("actor-submit.json");fs::write(&path,encode_command(&proposal(&c)).unwrap()).unwrap();
    let peer=actor_submit(actor.clone(),path); let human=human(reviewer.clone());
    serve(c,&actor,&reviewer,None,false,||ElapsedTick(1000)).unwrap();
    human.join().unwrap();let response=peer.join().unwrap();assert!(matches!(response.result,Ok(Knowledge::Known{value:ActorOutcome::Executed,..})));
    let c=configured(&root);let disk=FileOversight::read_publication(&c.store,&c.profile).unwrap();
    assert_eq!(disk.executions,1);assert_eq!(disk.payload,b"network");assert_eq!(disk.control.ledger.charged,7);assert_eq!(disk.control.ledger.reserved,0);
    assert!(!actor.socket.exists());
}

#[test]
fn actor_can_cancel_while_human_is_absent_without_stopping_other_authority() {
    let root=Directory::new();let c=configured(&root);evidence(&root,&c);let (actor,reviewer)=profiles(&root,&c);
    let command=proposal(&c);let p=actor.clone();let review_socket=reviewer.socket(7);
    let peer=std::thread::spawn(move||{
        let mut budget=ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let (stream,response)=exchange(connect(&p),command,&mut budget);assert!(matches!(response.result,Ok(Knowledge::Pending{..})));
        wait(&review_socket);
        let (_,response)=exchange(stream,Command::Cancel{request:7},&mut budget);response
    });
    serve(c,&actor,&reviewer,None,false,||ElapsedTick(1000)).unwrap();
    assert!(matches!(peer.join().unwrap().result,Ok(Knowledge::Known{value:ActorOutcome::CancelledBeforeDispatch,..})));
    let c=configured(&root);let disk=FileOversight::read_publication(&c.store,&c.profile).unwrap();
    assert_eq!(disk.executions,0);assert_eq!(disk.control.ledger.reserved,0);assert_eq!(disk.control.ledger.charged,0);assert!(disk.stop.is_none());
}

#[test]
fn historical_exact_retry_needs_no_evidence_helpers_or_reviewer_connection() {
    let root=Directory::new();let c=configured(&root);evidence(&root,&c);let (actor,reviewer)=profiles(&root,&c);
    let path=root.0.join("actor-submit.json");fs::write(&path,encode_command(&proposal(&c)).unwrap()).unwrap();
    let peer=actor_submit(actor.clone(),path.clone());let human=human(reviewer.clone());
    serve(c,&actor,&reviewer,None,false,||ElapsedTick(1000)).unwrap();peer.join().unwrap();human.join().unwrap();
    fs::remove_file(root.0.join("evidence.json")).unwrap();let mut c=configured(&root);c.programs.clear();
    let peer=actor_submit(actor.clone(),path);
    serve(c,&actor,&reviewer,None,true,||ElapsedTick(200000)).unwrap();
    assert!(matches!(peer.join().unwrap().result,Ok(Knowledge::Known{value:ActorOutcome::Executed,..})));
    let c=configured(&root);let disk=FileOversight::read_publication(&c.store,&c.profile).unwrap();assert_eq!(disk.executions,1);assert_eq!(disk.control.ledger.charged,7);
    assert!(!reviewer.socket(7).exists());
}

#[test]
fn socket_collision_and_foreign_scope_do_not_create_or_replace_a_journal() {
    let root=Directory::new();let c=configured(&root);let (mut actor,reviewer)=profiles(&root,&c);
    actor.scope.run+=1;assert!(serve(c,&actor,&reviewer,None,false,||ElapsedTick(1000)).is_err());assert!(!root.0.join("store").exists());
    let c=configured(&root);let (actor,reviewer)=profiles(&root,&c);fs::write(&actor.socket,b"do not replace").unwrap();
    assert!(serve(c,&actor,&reviewer,None,false,||ElapsedTick(1000)).is_err());assert_eq!(fs::read(&actor.socket).unwrap(),b"do not replace");
    assert!(!root.0.join("store").exists());
}

#[test]
fn actor_profile_requires_exact_roles_bounded_work_and_explicit_pid_policy() {
    let root=Directory::new();let c=configured(&root);let valid=actor_json(&root,&c);assert!(Profile::decode(valid.as_bytes()).is_ok());
    for invalid in [valid.replace("\"request\":7","\"request\":0"),valid.replace("\"candidate_limit\":8","\"candidate_limit\":0"),
        valid.replace("\"connection_limit\":4","\"connection_limit\":9"),valid.replace("\"exchange_limit\":1024","\"exchange_limit\":0"),
        valid.replace("\"poll_ms\":1","\"poll_ms\":0"),valid.replace("unix_milliseconds","saved_ticks"),
        valid.replace("\"request\":7","\"request\":7,\"extra\":true"),valid.replace("\"request\":7","\"request\":7,\"request\":8"),
        valid.replace("/actors/actor.sock","/actors/../actor.sock"),
        valid.replace("\"tenant\":1", "\"tenant\":0")] {
        assert!(Profile::decode(invalid.as_bytes()).is_err(),"{invalid}");
    }
}

#[test]
fn wrong_actor_process_policy_receives_no_protocol_bytes_or_admission() {
    use std::io::Read;
    use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
    let root=Directory::new();let c=configured(&root);let (mut actor,reviewer)=profiles(&root,&c);
    let id=credentials();actor.actor=PeerPolicy::new(id.uid(),id.gid(),Some(id.pid()+1)).unwrap();actor.candidates=1;actor.connections=1;
    let p=actor.clone();let peer=std::thread::spawn(move||{
        wait(&p.socket);let mut socket=UnixStream::connect(&p.socket).unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut one=[0];match socket.read(&mut one) { Ok(0)=>{}, Err(e) if e.kind()==io::ErrorKind::ConnectionReset=>{}, other=>panic!("unexpected disclosure: {other:?}") }
    });
    assert!(serve(c,&actor,&reviewer,None,false,||ElapsedTick(1000)).is_err());peer.join().unwrap();
    let c=configured(&root);let disk=FileOversight::read_publication(&c.store,&c.profile).unwrap();
    assert!(disk.control.ledger.stages.is_empty());assert_eq!(disk.executions,0);assert_eq!(disk.control.ledger.charged,0);
}

#[test]
fn actor_client_authenticates_server_before_sending_a_document() {
    use std::io::Read;
    use std::os::unix::net::UnixListener;
    use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
    let root=Directory::new();let c=configured(&root);let (mut actor,_)=profiles(&root,&c);
    let path=root.0.join("document.json");fs::write(&path,encode_command(&proposal(&c)).unwrap()).unwrap();
    let id=credentials();actor.supervisor=PeerPolicy::new(id.uid(),id.gid(),Some(id.pid()+1)).unwrap();
    let listener=UnixListener::bind(&actor.socket).unwrap();
    let peer=std::thread::spawn(move||client::submit(&actor,&path,&mut Vec::new()));
    let (mut socket,_)=listener.accept().unwrap();socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    assert_eq!(socket.read(&mut [0;1]).unwrap(),0);assert!(peer.join().unwrap().is_err());assert!(!c.store.exists());
}

#[test]
fn checked_live_actor_revalidates_whole_input_at_the_final_publication_boundary() {
    use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationInputs;
    use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{FilePublicationProducer,PublicationProducerProfile};
    use fa_reference::full_input::{ActualHelperInput,ByteSpan,InputProfileBinding,PartKind,SubmittedPart};
    for changed in [false,true] {
        let root=Directory::new();let c=configured(&root);evidence(&root,&c);let (actor,reviewer)=profiles(&root,&c);
        let actual=|epoch| ActualHelperInput::new(b"Q".to_vec(),InputProfileBinding {profile_id:1,profile_bytes:vec![],tokenizer_epoch:1,policy_epoch:0,model_epoch:epoch},
            vec![SubmittedPart{span:ByteSpan{start:0,end:1},kind:PartKind::Question}],vec![]).unwrap();
        let producer_profile=PublicationProducerProfile {source:91,scope:c.profile.delivery.scope,feed:41,clock_domain:CLOCK_DOMAIN,after:0};
        let (mut producer,_)=FilePublicationProducer::create(root.0.join("producer"),producer_profile,FilePublicationInputs::new(None,Some(actual(1))),ElapsedTick(1000)).unwrap();
        let text=format!(r#"{{"schema":"fa.supervised-whole-input/1","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[]}}"#,root.0.display(),scope(&c));
        let checked=PublicationProfile::decode(text.as_bytes()).unwrap();
        let path=root.0.join("actor-submit.json");fs::write(&path,encode_command(&proposal(&c)).unwrap()).unwrap();
        let peer=actor_submit(actor.clone(),path);let human=human(reviewer.clone());let store=c.store.clone();let bootstrap=c.profile.clone();let mut advanced=false;
        let result=serve(c,&actor,&reviewer,Some(&checked),false,||{
            if !advanced && let Ok(disk)=FileOversight::read_publication(&store,&bootstrap)
                && disk.control.ledger.charged!=0 && disk.executions==0 {
                producer.publish(1,FilePublicationInputs::new(None,Some(actual(if changed {2}else{1}))),ElapsedTick(1000)).unwrap();advanced=true;
            }
            ElapsedTick(1000)
        });
        if !changed {assert!(result.is_ok(),"{result:?}");}
        human.join().unwrap();let response=peer.join().unwrap();assert!(advanced);
        assert_eq!(matches!(response.result,Ok(Knowledge::Known{value:ActorOutcome::Executed,..})),!changed);
        let disk=FileOversight::read_publication(&store,&bootstrap).unwrap();assert_eq!(disk.executions,u64::from(!changed));
        assert_eq!(disk.control.ledger.charged,if changed{0}else{7});assert_eq!(disk.control.ledger.reserved,0);
    }
}

#[test]
fn failed_actor_output_does_not_retransmit_an_executed_request() {
    use std::io::Write;
    struct Broken;
    impl Write for Broken {
        fn write(&mut self,_:&[u8])->io::Result<usize>{Err(io::ErrorKind::BrokenPipe.into())}
        fn flush(&mut self)->io::Result<()>{Err(io::ErrorKind::BrokenPipe.into())}
    }
    let root=Directory::new();let c=configured(&root);evidence(&root,&c);let (actor,reviewer)=profiles(&root,&c);
    let path=root.0.join("actor-submit.json");fs::write(&path,encode_command(&proposal(&c)).unwrap()).unwrap();
    let p=actor.clone();let peer=std::thread::spawn(move||{wait(&p.socket);client::submit(&p,&path,&mut Broken)});let human=human(reviewer.clone());
    serve(c,&actor,&reviewer,None,false,||ElapsedTick(1000)).unwrap();human.join().unwrap();
    assert!(peer.join().unwrap().unwrap_err().contains("native actor response received, output failed"));
    let c=configured(&root);let disk=FileOversight::read_publication(&c.store,&c.profile).unwrap();assert_eq!(disk.executions,1);assert_eq!(disk.control.ledger.charged,7);
}

#[path = "tests/series.rs"]
mod sequential;
