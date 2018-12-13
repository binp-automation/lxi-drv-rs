use std::io;

use mio::*;

use ::channel::*;


pub type ProxyId = usize;
pub type ProxyPort = usize;

pub const TOKEN_PORT_BITS: usize = 8;
pub const TOKEN_PORT_MASK: usize = (1<<TOKEN_PORT_BITS) - 1;

#[derive(Debug)]
pub enum TokenError {
    BadId(ProxyId),
    BadPort(ProxyPort),
}

pub fn encode_token(id: ProxyId, port: ProxyPort) -> Result<Token, TokenError> {
    let base = id << TOKEN_PORT_BITS;
    if base >> TOKEN_PORT_BITS != id {
        return Err(TokenError::BadId(id));
    }
    if port & TOKEN_PORT_MASK != port {
        return Err(TokenError::BadPort(port));
    }
    Ok(Token(base | port))
}

pub fn decode_token(token: &Token) -> (ProxyId, ProxyPort) {
    let id = token.0 >> TOKEN_PORT_BITS;
    let port = token.0 & TOKEN_PORT_MASK;
    (id, port)
}

#[derive(Debug)]
pub enum ProxyError {
    Token(TokenError),
    Io(io::Error),
    Chan(ChanError),
}

impl From<TokenError> for ProxyError {
    fn from(te: TokenError) -> Self {
        ProxyError::Token(te)
    }
}

impl From<io::Error> for ProxyError {
    fn from(ioe: io::Error) -> Self {
        ProxyError::Io(ioe)
    }
}

#[derive(Debug)]
pub enum ProxyAction {
    Detach,
    Nope,
}

pub trait Proxy {
    fn register(&mut self, poll: &Poll, id: ProxyId) -> Result<(), ProxyError>;
    fn deregister(&mut self, poll: &Poll) -> Result<(), ProxyError>;

    fn process(&mut self, poll: &Poll, event: &Event) -> Result<ProxyAction, ProxyError>;
}


#[cfg(test)]
mod tests {
    use super::*;

    use std::mem;


    #[test]
    fn encode() {
        assert_eq!(
            (456 << TOKEN_PORT_BITS) | 123,
            encode_token(456, 123).unwrap().0
        );

        assert_matches!(
            encode_token(0, (1<<TOKEN_PORT_BITS)+1),
            Err(TokenError::BadPort(_))
        );

        assert_matches!(
            encode_token(1<<(8*mem::size_of::<usize>()-1), 0),
            Err(TokenError::BadId(_))
        );
    }

    #[test]
    fn decode() {
        assert_eq!(
            decode_token(&Token((456 << TOKEN_PORT_BITS) | 123)),
            (456, 123)
        );
    }
}
