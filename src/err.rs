use std::error::Error;

pub type WhyResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
