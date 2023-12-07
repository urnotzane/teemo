use url::Url;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use serde_json::Value;
use reqwest::Method;

mod utils;

#[derive(Debug)]
pub struct Teemo {
    app_token: String,
    app_port: i32,
    url: Url,
    ws_url: Url,
    closed: bool,
}

impl Teemo {
    pub fn new() -> Teemo {
        Teemo {
            app_token: String::from(""),
            app_port: 0,
            url: Url::parse("https://127.0.0.1").unwrap(),
            ws_url: Url::parse("wss://127.0.0.1").unwrap(),
            closed: false,
        }
    }

    pub fn start(&mut self) {
        self.closed = false;
        self.initialize();
    }

    pub fn close(&mut self) {
        self.closed = true;
        println!("Teemo is closed.");
    }

    fn initialize(&mut self) {
        while self.closed == false {

            let remote_data = utils::get_lcu_cmd_data();

            if remote_data.len() < 2 {
                println!("LCU is not running.");
                thread::sleep(Duration::from_millis(500));
                continue;
            }

            self.app_token = remote_data.get("remoting-auth-token").unwrap().to_owned();
            self.app_port = remote_data
                .get("app-port")
                .unwrap()
                .to_owned()
                .parse::<i32>()
                .unwrap();
            self.url = Url::parse(&("https://127.0.0.1:".to_string() + &self.app_port.to_string()))
                .unwrap();
            self.ws_url =
                Url::parse(&("wss://127.0.0.1:".to_string() + &self.app_port.to_string())).unwrap();
            
            println!("Teemo has finished initializing.LCU is running on {}", self.url);
            println!("LCU is running on {}, token: {}", self.url, self.app_token);
            break;
        }
    }

    /// 用来发送LCU请求
    pub async fn request(
        &self,
        method: &str,
        url: &str,
        data: Option<HashMap<String, Value>>,
    ) -> Result<HashMap<String, Value>, serde_json::Error> {
        // if self.app_token.len() < 1 {
        //     return Ok(HashMap::new());
        // }

        let method_byte = method.as_bytes();
        let client: reqwest::Client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .no_proxy()
            .build()
            .unwrap();
    
        let response = client
            .request(
                Method::from_bytes(method_byte).unwrap(),
                format!("{}{}", self.url, url),
            )
            .basic_auth("riot", Some(&self.app_token))
            .json(&data)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        
        Ok(serde_json::from_str(&response).unwrap())
    }
}
