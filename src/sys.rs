use super::err::WhyResult;
use std::path::Path;

pub fn just_give_me_str_path(path: &Path) -> WhyResult<&str> {
    let filename: &str = path.to_str().ok_or(format!("Invalid path: {:?}", path))?;
    Ok(filename)
}
