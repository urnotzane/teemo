use url::Url;
use std::thread;
use std::time::Duration;

mod utils;

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
            ws_url: Url::parse("https://127.0.0.1").unwrap(),
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
            
            println!("Teemo has finished initializing.");
            break;
        }
    }
}
