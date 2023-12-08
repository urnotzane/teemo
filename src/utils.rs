use reqwest::{self, Client};
#[cfg(windows)]
use std::{collections::HashMap, os::windows::process::CommandExt, process::Command};

#[cfg(windows)]
pub fn execute_command(cmd_str: &str) -> String {
    let output = Command::new("cmd")
        // 运行cmd时隐藏窗口
        .creation_flags(0x08000000)
        .args(["/C", cmd_str])
        .output()
        .expect("failed to execute process");

    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn get_lcu_cmd_data() -> HashMap<String, String> {
    let data_str = execute_command("wmic PROCESS WHERE name='LeagueClientUx.exe' GET commandline");
    format_lcu_data(data_str)
}

fn format_lcu_data(data_str: String) -> HashMap<String, String> {
    let mut data_map = HashMap::new();
    let col: Vec<_> = data_str.split("\"--").collect();
    if col.len() < 2 {
        return data_map;
    }
    for item in col {
        let temp: Vec<_> = item.split("=").collect();
        if temp.len() < 2 {
            continue;
        }
        data_map.insert(
            temp[0].to_string(),
            temp[1].replace("\"", "").trim().to_string(),
        );
    }
    data_map
}
pub fn create_client() -> Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .build()
        .unwrap()
}

#[test]
fn it_works() {
    let result = format_lcu_data(
        "\"--remoting-auth-token=_BkC3zoDF6600gmlQdUs6w\" \"--app-port=58929\"".to_string(),
    );
    let mut assert_res = HashMap::new();
    assert_res.insert(
        "remoting-auth-token".to_string(),
        "_BkC3zoDF6600gmlQdUs6w".to_string(),
    );
    assert_res.insert("app-port".to_string(), "58929".to_string());
    assert_eq!(result, assert_res);
}
