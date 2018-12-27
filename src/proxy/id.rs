use mio::{Token};


pub type Id = usize;
pub type Eid = usize;

pub const EID_BITS: usize = 8;
pub const EID_MASK: usize = (1<<EID_BITS) - 1;

pub fn encode_ids(id: Id, eid: Eid) -> Option<Token> {
    let base = id << EID_BITS;
    if base >> EID_BITS != id {
        return None;
    }
    if eid & EID_MASK != eid {
        return None;
    }
    Some(Token(base | eid))
}

pub fn decode_ids(token: Token) -> (Id, Eid) {
    let id = token.0 >> EID_BITS;
    let eid = token.0 & EID_MASK;
    (id, eid)
}


#[cfg(test)]
mod test {
    use super::*;

    use std::mem;


    #[test]
    fn ids_encode() {
        assert_eq!(
            (456 << EID_BITS) | 123,
            encode_ids(456, 123).unwrap().0
        );

        assert!(encode_ids(0, (1<<EID_BITS)+1).is_none());
        assert!(encode_ids(1<<(8*mem::size_of::<usize>()-1), 0).is_none());
    }

    #[test]
    fn ids_decode() {
        assert_eq!(
            decode_ids(Token((456 << EID_BITS) | 123)),
            (456, 123)
        );
    }
}
