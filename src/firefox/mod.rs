//! Firefox family code: finding profiles in `profiles.ini` and reading the cookie database.

pub(crate) mod database;
pub(crate) mod profiles_ini;
#[cfg(test)]
pub(crate) mod test_data;
