fn main() {
    println!("cargo:rerun-if-changed=config.toml");

    let config_content = std::fs::read_to_string("config.toml").unwrap_or_default();

    // Parse TOML config
    let config: toml::Table = match toml::from_str(&config_content) {
        Ok(t) => t,
        Err(_) => {
            println!("cargo:rerun-if-changed=build.rs");
            return;
        }
    };

    // Get plugin states from [plugins.plugin_states]
    if let Some(plugins) = config.get("plugins").and_then(|p| p.as_table()) {
        if let Some(states) = plugins.get("plugin_states").and_then(|s| s.as_table()) {
            for (name, state) in states {
                if let Some(state_str) = state.as_str() {
                    if state_str == "static" {
                        match name.as_str() {
                            "plot" => println!("cargo:rustc-cfg=feature=\"plugin-plot\""),
                            "png" => println!("cargo:rustc-cfg=feature=\"plugin-png\""),
                            "sql" => println!("cargo:rustc-cfg=feature=\"plugin-sql\""),
                            "fft" => println!("cargo:rustc-cfg=feature=\"plugin-fft\""),
                            "python" => println!("cargo:rustc-cfg=feature=\"plugin-python\""),
                            "gtk" => println!("cargo:rustc-cfg=feature=\"plugin-gtk\""),
                            "cdr" => println!("cargo:rustc-cfg=feature=\"plugin-cdr\""),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    println!("cargo:rerun-if-changed=build.rs");
}
