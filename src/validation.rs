use lazy_regex::regex_is_match;

use crate::error::Error;
use crate::error::Result;

/// First character is a letter, username is between 3 and 30 characters long,
/// and is alphanumeric.
pub fn username(username: &str) -> Result<()> {
    if !(username.chars().next().is_some_and(|x| x.is_lowercase())
        && username.len() >= 3
        && username.len() < 30
        && regex_is_match!(r"^[0-9A-Za-z]+$", username))
    {
        Err(Error::AppError(format!("Bad username \"{}\"", username)))
    } else {
        Ok(())
    }
}
