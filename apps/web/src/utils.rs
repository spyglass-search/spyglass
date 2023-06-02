use js_sys::decode_uri_component;

pub fn decode_string(value: &str) -> String {
    if let Ok(Some(decoded)) = decode_uri_component(value).map(|x| x.as_string()) {
        decoded
    } else {
        value.to_owned()
    }
}

pub fn validate_hex_color(hex_color: &str) -> Result<(), String> {
    // Check if the hex color is valid.
    if hex_color.len() != 3 && hex_color.len() != 6 {
        return Err("Invalid hex color length.".to_string());
    }

    // Check if the hex color contains only valid characters.
    for c in hex_color.chars() {
        if !c.is_ascii_hexdigit() {
            return Err("Invalid hex color character.".to_string());
        }
    }

    // The hex color is valid.
    Ok(())
}
