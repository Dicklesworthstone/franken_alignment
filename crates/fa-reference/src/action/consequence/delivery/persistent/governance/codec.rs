//! Bounded exact policy syntax in the existing journal, not a new evaluator.
use super::PolicyUpdate;
use super::super::codec::shared::{Reader, Writer};
use crate::action::ResolvedTarget;
use crate::action::consequence::gate::containment::session::policy::{
    Policy, Predicate, MAX_POLICY_NODES, MAX_POLICY_EDGES, MAX_POLICY_LITERAL_BYTES,
};
use crate::Error;

// Includes worst-case legal literals, graph edges and fixed node fields.
pub(in super::super) const MAX_UPDATE_BYTES: usize = 128 * 1024;

pub(in super::super) fn encode(update: &PolicyUpdate) -> Result<Vec<u8>, Error> {
    let mut w = Writer::new(MAX_UPDATE_BYTES);
    for value in [update.operation(), update.expected_control_sequence(),
        update.expected_authority_epoch(), update.policy().generation()] { w.u64(value)?; }
    w.count(update.policy().nodes().len())?;
    for node in update.policy().nodes() {
        match node {
            Predicate::TargetIs(t) => {
                w.u8(0)?;
                for value in [t.adapter, t.object, t.contract_version, t.expected_version, t.generation] { w.u64(value)?; }
            }
            Predicate::PayloadIs(value) => { w.u8(1)?; w.blob(value)?; }
            Predicate::PayloadAtMost(limit) => { w.u8(2)?; w.u64(u64::try_from(*limit).map_err(|_| Error::Limit)?)?; }
            Predicate::UnitsAtMost(limit) => { w.u8(3)?; w.u64(*limit)?; }
            Predicate::ExactValue { key, value } => { w.u8(4)?; w.u64(*key)?; w.blob(value)?; }
            Predicate::Absent { key } => { w.u8(5)?; w.u64(*key)?; }
            Predicate::EmptyRange { start, end } => { w.u8(6)?; w.u64(*start)?; w.u64(*end)?; }
            Predicate::All(children) | Predicate::Any(children) => {
                w.u8(if matches!(node, Predicate::All(_)) { 7 } else { 8 })?;
                w.count(children.len())?;
                for child in children { w.u64(u64::try_from(*child).map_err(|_| Error::Limit)?)?; }
            }
            Predicate::Not(child) => { w.u8(9)?; w.u64(u64::try_from(*child).map_err(|_| Error::Limit)?)?; }
        }
    }
    Ok(w.finish())
}

fn index(r: &mut Reader<'_>) -> Result<usize, Error> {
    usize::try_from(r.u64()?).map_err(|_| Error::Limit)
}
fn literal(r: &mut Reader<'_>, remaining: &mut usize) -> Result<Vec<u8>, Error> {
    let bytes = r.blob(*remaining)?;
    *remaining -= bytes.len();
    Ok(bytes.to_vec())
}

pub(in super::super) fn decode(bytes: &[u8]) -> Result<PolicyUpdate, Error> {
    if bytes.len() > MAX_UPDATE_BYTES { return Err(Error::Limit); }
    let mut r = Reader::new(bytes);
    let operation = r.u64()?;
    let sequence = r.u64()?;
    let epoch = r.u64()?;
    let generation = r.u64()?;
    let count = r.count(MAX_POLICY_NODES)?;
    let mut nodes = Vec::new();
    nodes.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    let mut literals_left = MAX_POLICY_LITERAL_BYTES;
    let mut edges_left = MAX_POLICY_EDGES;
    for _ in 0..count {
        let tag = r.u8()?;
        nodes.push(match tag {
            0 => Predicate::TargetIs(ResolvedTarget { adapter: r.u64()?, object: r.u64()?,
                contract_version: r.u64()?, expected_version: r.u64()?, generation: r.u64()? }),
            1 => Predicate::PayloadIs(literal(&mut r, &mut literals_left)?),
            2 => Predicate::PayloadAtMost(index(&mut r)?),
            3 => Predicate::UnitsAtMost(r.u64()?),
            4 => Predicate::ExactValue { key: r.u64()?, value: literal(&mut r, &mut literals_left)? },
            5 => Predicate::Absent { key: r.u64()? },
            6 => Predicate::EmptyRange { start: r.u64()?, end: r.u64()? },
            7 | 8 => {
                let length = r.count(edges_left)?;
                edges_left -= length;
                let mut children = Vec::new();
                children.try_reserve_exact(length).map_err(|_| Error::Limit)?;
                for _ in 0..length { children.push(index(&mut r)?); }
                if tag == 7 { Predicate::All(children) } else { Predicate::Any(children) }
            }
            9 => {
                edges_left = edges_left.checked_sub(1).ok_or(Error::Limit)?;
                Predicate::Not(index(&mut r)?)
            }
            _ => return Err(Error::InvalidInput),
        });
    }
    r.end()?;
    // The ORIGINAL constructor checks topological order, root reachability,
    // negative-range validity, source-read limits and all target fields.
    let update = PolicyUpdate::new(operation, sequence, epoch, Policy::new(generation, nodes)?)?;
    if encode(&update)?.as_slice() != bytes { return Err(Error::Binding); }
    Ok(update)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_original_predicates_roundtrip_without_replacing_the_evaluator() {
        let t = ResolvedTarget { adapter: 1, object: 2, contract_version: 3, expected_version: 4, generation: 5 };
        let nodes = vec![Predicate::TargetIs(t), Predicate::PayloadIs(vec![0, 255]),
            Predicate::PayloadAtMost(65536), Predicate::UnitsAtMost(u64::MAX),
            Predicate::ExactValue { key: u64::MAX, value: vec![128] }, Predicate::Absent { key: 9 },
            Predicate::EmptyRange { start: 10, end: u64::MAX }, Predicate::Not(5),
            Predicate::Any(vec![6, 7]), Predicate::All(vec![0, 1, 2, 3, 4, 8])];
        let update = PolicyUpdate::new(u64::MAX, 123, 456, Policy::new(7, nodes).unwrap()).unwrap();
        let bytes = encode(&update).unwrap();
        assert_eq!(decode(&bytes).unwrap(), update);
        for end in 0..bytes.len() { assert!(decode(&bytes[..end]).is_err()); }
        let mut extra = bytes.clone(); extra.push(0); assert!(decode(&extra).is_err());
    }

    #[test]
    fn invalid_graphs_tags_and_aggregate_literals_refuse_before_becoming_a_policy() {
        let update = PolicyUpdate::new(1, 2, 3, Policy::new(4, vec![Predicate::Absent { key: 1 }]).unwrap()).unwrap();
        let bytes = encode(&update).unwrap();
        let mut unknown = bytes.clone(); unknown[36] = 255; assert!(decode(&unknown).is_err());
        let mut cycle = bytes.clone(); cycle[36] = 9;
        cycle[37..45].copy_from_slice(&0_u64.to_be_bytes()); assert!(decode(&cycle).is_err());
        let mut zero = bytes.clone(); zero[..8].fill(0); assert!(decode(&zero).is_err());
        let mut many = bytes; many[32..36].copy_from_slice(&129_u32.to_be_bytes());
        assert_eq!(decode(&many), Err(Error::Limit));
        let mut w = Writer::new(MAX_UPDATE_BYTES);
        for n in [1, 0, 0, 1] { w.u64(n).unwrap(); }
        w.count(3).unwrap();
        w.u8(1).unwrap(); w.blob(&vec![0; MAX_POLICY_LITERAL_BYTES]).unwrap();
        w.u8(1).unwrap(); w.blob(&[1]).unwrap();
        w.u8(7).unwrap(); w.count(2).unwrap(); w.u64(0).unwrap(); w.u64(1).unwrap();
        assert_eq!(decode(&w.finish()), Err(Error::Limit));
    }
}
