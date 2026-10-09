use crate::domain::params::PowParams;

#[derive(Clone, Copy, Debug)]
pub struct LaunchParams {
    pub time_cost: i32,
    pub mem_cost: i32,
    pub parallelism: i32,
    pub version: i32,
    pub key_length: i32,
}

impl LaunchParams {
    pub fn from_params(params: &PowParams) -> Self {
        Self {
            time_cost: params.time_cost as i32,
            mem_cost: params.mem_cost as i32,
            parallelism: params.parallelism as i32,
            version: params.version_raw as i32,
            key_length: params.key_length as i32,
        }
    }
}
