use serde::{Deserialize, Serialize};


const DEFAULT_CONFIG: &str = include_str!("default.toml");
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Config{
    pub server: ServerConfig,
    pub speech_to_text: SpeechToTextConfig,
    pub dialogue_model: DialogueModelConfig,
    pub text_to_speech: TextToSpeechConfig,
}
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ServerConfig{    
    pub addr: Option<String>

}
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SpeechToTextConfig{
    pub model_path: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DialogueModelConfig{
    pub model_path: Option<String>,
    pub model_name: Option<String>,
    pub api_key:Option<String>,
    pub stream: bool,
    pub prompt_path: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TextToSpeechConfig{
    pub model_path: Option<String>,
}

impl Config{
    pub fn new() -> Self{
        toml::from_str(DEFAULT_CONFIG).unwrap()
    }

}

#[cfg(test)]

mod test{
    use super::*;
    use std::fs;
    #[test]
    fn test_config(){
        let config = Config::new();
        println!("{:?}",config);
    }
}