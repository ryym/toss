use std::error::Error;

pub(crate) type AnyError = Box<dyn Error>;
