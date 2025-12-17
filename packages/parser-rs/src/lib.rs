use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Token {
    pub token_type: String,
    pub value: String,
}

#[derive(Serialize, Deserialize)]
pub struct ParseResult {
    pub tokens: Vec<Token>,
    pub html: String,
}

/// Parse MDM syntax to tokens
#[wasm_bindgen]
pub fn parse_mdm(input: &str) -> JsValue {
    let tokens = tokenize(input);
    let html = render_tokens(&tokens);
    
    let result = ParseResult { tokens, html };
    
    serde_wasm_bindgen::to_value(&result).unwrap()
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current_pos = 0;
    
    while current_pos < input.len() {
        let remaining = &input[current_pos..];
        
        // Look for MDM reference: ![[...]]
        if remaining.starts_with("![[") {
            if let Some(end_pos) = remaining.find("]]") {
                let content = &remaining[3..end_pos];
                tokens.push(Token {
                    token_type: "mdm-reference".to_string(),
                    value: content.to_string(),
                });
                current_pos += end_pos + 2;
                continue;
            }
        }
        
        // Regular text
        let next_mdm = remaining.find("![[").unwrap_or(remaining.len());
        if next_mdm > 0 {
            tokens.push(Token {
                token_type: "text".to_string(),
                value: remaining[..next_mdm].to_string(),
            });
            current_pos += next_mdm;
        } else {
            current_pos += 1;
        }
    }
    
    tokens
}

fn render_tokens(tokens: &[Token]) -> String {
    let mut html = String::new();
    
    for token in tokens {
        match token.token_type.as_str() {
            "text" => html.push_str(&token.value),
            "mdm-reference" => {
                // Parse reference
                let parts: Vec<&str> = token.value.split('|').collect();
                let filename = parts[0].trim();
                
                // Simple image rendering
                html.push_str(&format!("<img src=\"{}\" alt=\"{}\">", filename, filename));
            }
            _ => {}
        }
    }
    
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize() {
        let input = "Hello ![[image.jpg]] world";
        let tokens = tokenize(input);
        
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].token_type, "text");
        assert_eq!(tokens[1].token_type, "mdm-reference");
        assert_eq!(tokens[2].token_type, "text");
    }
}
