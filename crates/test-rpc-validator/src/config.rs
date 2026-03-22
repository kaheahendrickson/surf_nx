use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ProgramToLoad {
    pub program_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub programs: Vec<String>,
}

impl Config {
    pub fn parse_programs(&self) -> Result<Vec<ProgramToLoad>, String> {
        let mut result = Vec::new();
        for program_str in &self.programs {
            let parts: Vec<&str> = program_str.splitn(2, '=').collect();
            if parts.len() != 2 {
                return Err(format!(
                    "Invalid program format '{}'. Expected PUBKEY=PATH",
                    program_str
                ));
            }
            result.push(ProgramToLoad {
                program_id: parts[0].to_string(),
                path: PathBuf::from(parts[1]),
            });
        }
        Ok(result)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8899,
            programs: Vec::new(),
        }
    }
}
