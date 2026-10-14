use super::*;
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use std::{io::Write as _, net::Shutdown, os::unix::net::UnixStream, sync::atomic::{AtomicU64, Ordering}};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path=std::env::temp_dir().join(format!("fa-evaluator-peer-{}-{stamp}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Directory { fn drop(&mut self){ let _=fs::remove_dir_all(&self.0); } }

fn config(root:&Directory)->Config {
    let mut c=Config::decode(include_bytes!("../../fixtures/supervised_publication.json")).unwrap();
    c.store=root.0.join("store"); c
}
fn settings()->Settings {
    Settings::decode(include_bytes!("../../fixtures/joint_native_andon_operator.json")).unwrap()
}
fn profile_json(root:&Directory, config:&Config, settings:&Settings, pid:Option<u32>)->Vec<u8> {
    let meta=fs::metadata(&root.0).unwrap();
    let id=settings.config.identity();
    let pid=pid.map_or("null".to_owned(),|p|p.to_string());
    format!(r#"{{"schema_version":"fa.native-evaluator-peer/1","socket":"{}","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"deployment":{},"evaluator":{},"supervisor":{{"uid":{},"gid":{},"pid":{}}},"peer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":3,"runtime_ms":1000,"poll_ms":1}}"#,
        root.0.join("eval.sock").display(),config.profile.delivery.scope.tenant,config.profile.delivery.scope.principal,
        config.profile.delivery.scope.run,config.profile.delivery.scope.branch,config.profile.delivery.scope.authority,
        id.deployment,id.evaluator,meta.uid(),meta.gid(),pid,meta.uid(),meta.gid(),pid).into_bytes()
}

#[test]
fn strict_profile_binds_scope_and_native_accounting_identity() {
    let root=Directory::new(); let config=config(&root); let settings=settings();
    let p=EvaluatorPeerProfile::decode(&profile_json(&root,&config,&settings,Some(std::process::id()))).unwrap();
    p.check(&config,&settings).unwrap();
    let text=String::from_utf8(profile_json(&root,&config,&settings,Some(std::process::id()))).unwrap();
    for changed in [
        text.replacen("fa.native-evaluator-peer/1","fa.native-evaluator-peer/2",1),
        text.replacen(&format!("\"deployment\":{}",settings.config.identity().deployment),"\"deployment\":999",1),
        text.replacen(&format!("\"authority\":{}",config.profile.delivery.scope.authority),"\"authority\":999",1),
        text.replacen("\"candidate_limit\":3","\"candidate_limit\":0",1),
        text.replacen("\"poll_ms\":1","\"poll_ms\":2000",1),
    ] { assert!(EvaluatorPeerProfile::decode(changed.as_bytes()).and_then(|p|p.check(&config,&settings)).is_err()); }
}

#[test]
fn peer_credentials_are_checked_before_any_assessment_bytes_are_consumed() {
    let (mut client,server)=UnixStream::pair().unwrap();
    let observed=PeerCredentials::observe(&server).unwrap();
    client.write_all(b"{\"secret\":true}").unwrap();
    let wrong=PeerPolicy::new(observed.uid(),observed.gid(),Some(observed.pid().saturating_add(1))).unwrap();
    assert_eq!(wrong.verify(&server).unwrap_err().kind(),std::io::ErrorKind::PermissionDenied);
    server.set_nonblocking(true).unwrap();
    let mut one=[0u8;1];
    assert_eq!((&server).read(&mut one).unwrap(),1);
    assert_eq!(one[0],b'{');
}

#[test]
fn bounded_document_reader_accepts_exact_input_and_refuses_oversize() {
    for size in [1usize,ASSESSMENT_BYTES,ASSESSMENT_BYTES+1] {
        let (mut client,mut server)=UnixStream::pair().unwrap();
        let handle=std::thread::spawn(move || {
            client.write_all(&vec![b'x';size]).unwrap();
            client.shutdown(Shutdown::Write).unwrap();
        });
        let result=read_document(&mut server); handle.join().unwrap();
        assert_eq!(result.is_ok(),size<=ASSESSMENT_BYTES);
        if let Ok(bytes)=result { assert_eq!(bytes.len(),size); }
    }
}

#[test]
fn socket_binding_never_unlinks_existing_paths_and_cleans_its_own_socket() {
    let root=Directory::new(); let config=config(&root); let settings=settings();
    let p=EvaluatorPeerProfile::decode(&profile_json(&root,&config,&settings,Some(std::process::id()))).unwrap();
    fs::write(&p.socket,b"occupied").unwrap();
    assert!(BoundEvaluatorSocket::bind(&p).is_err());
    assert_eq!(fs::read(&p.socket).unwrap(),b"occupied");
    fs::remove_file(&p.socket).unwrap();
    { let _bound=BoundEvaluatorSocket::bind(&p).unwrap(); assert!(p.socket.exists()); }
    assert!(!p.socket.exists());
}

#[test]
fn exact_kernel_peer_policy_accepts_current_process_and_pid_drift_refuses() {
    let (_client,server)=UnixStream::pair().unwrap();
    let observed=PeerCredentials::observe(&server).unwrap();
    let exact=PeerPolicy::new(observed.uid(),observed.gid(),Some(observed.pid())).unwrap();
    assert_eq!(exact.verify(&server).unwrap(),observed);
    let drift=if observed.pid()<i32::MAX as u32 { observed.pid()+1 } else { observed.pid()-1 };
    let wrong=PeerPolicy::new(observed.uid(),observed.gid(),Some(drift)).unwrap();
    assert_eq!(wrong.verify(&server).unwrap_err().kind(),std::io::ErrorKind::PermissionDenied);
}
