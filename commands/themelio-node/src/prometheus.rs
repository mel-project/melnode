use crate::storage::NodeStorage;

use std::fmt;
use std::sync::RwLock;
use std::thread;
use std::time;

use once_cell::sync::{Lazy, OnceCell};
use prometheus::{
    labels, opts, register_gauge, register_int_gauge, Encoder, Gauge, IntGauge, Registry,
    TextEncoder,
};
use rweb::{get, serve};
use smol::Timer;
use smol_timeout::TimeoutExt;
use systemstat::platform::PlatformImpl;
use systemstat::{CPULoad, Memory, Platform, System};

// Complete list of metadata endpoints available here: https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/instancedata-data-categories.html
const AWS_API_TOKEN_URL: &str = "http://169.254.169.254/latest/api/token";

const AWS_INSTANCE_REGION_URL: &str = "http://169.254.169.254/latest/meta-data/placement/region";

const AWS_INSTANCE_ID_URL: &str = "http://169.254.169.254/latest/meta-data/instance-id";

#[derive(Debug)]
enum AWSError {
    APIToken,
    InstanceID,
    Region,
}

impl fmt::Display for AWSError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AWSError::APIToken => write!(formatter, "Failed to retrieve AWS API token."),
            AWSError::InstanceID => write!(formatter, "Failed to retrieve AWS instance ID."),
            AWSError::Region => write!(formatter, "Failed to retrieve AWS region."),
        }
    }
}

async fn aws_api_token() -> Result<String, AWSError> {
    let client: reqwest::Client = reqwest::Client::new();

    let aws_api_token_result: Result<reqwest::Response, reqwest::Error> = client
        .put(AWS_API_TOKEN_URL)
        .header("X-aws-ec2-metadata-token-ttl-seconds", "21600")
        .send()
        .await;

    match aws_api_token_result {
        Ok(aws_api_token_response) => match aws_api_token_response.text().await {
            Ok(token) => Ok(token),
            Err(error) => {
                log::error!("Could not retrieve token text: {}", error);

                Ok(String::from(""))
            }
        },
        Err(error) => {
            log::error!("Could not retrieve the AWS API token: {}", error);

            Err(AWSError::APIToken)
        }
    }
}

async fn aws_region() -> Result<String, AWSError> {
    let client: reqwest::Client = reqwest::Client::new();

    let aws_api_token: String = match aws_api_token().await {
        Ok(region) => {
            log::debug!("Successfully retrieved AWS region.");

            region
        }
        Err(error) => {
            log::error!("{}", error);

            String::from("")
        }
    };

    let region_request_result: Result<reqwest::Response, reqwest::Error> = client
        .get(AWS_INSTANCE_REGION_URL)
        .header("X-aws-ec2-metadata-token", aws_api_token)
        .send()
        .await;

    match region_request_result {
        Ok(region_request_response) => match region_request_response.text().await {
            Ok(region) => Ok(region),
            Err(error) => {
                log::error!("Could not retrieve region text: {}", error);

                Ok(String::from(""))
            }
        },
        Err(error) => {
            log::error!("{}", error);

            Err(AWSError::Region)
        }
    }
}

async fn aws_instance_id() -> Result<String, AWSError> {
    let client: reqwest::Client = reqwest::Client::new();

    let aws_api_token: String = match aws_api_token().await {
        Ok(token) => {
            log::debug!("Successfully retrieved AWS region.");

            token
        }
        Err(error) => {
            log::error!("{}", error);

            String::from("")
        }
    };

    let instance_id_request_result: Result<reqwest::Response, reqwest::Error> = client
        .get(AWS_INSTANCE_ID_URL)
        .header("X-aws-ec2-metadata-token", aws_api_token)
        .send()
        .await;

    match instance_id_request_result {
        Ok(instance_id_request_response) => {
            log::debug!("Successfully retrieved AWS instance ID.");

            match instance_id_request_response.text().await {
                Ok(instance_id) => Ok(instance_id),
                Err(error) => {
                    log::error!("Could not retrieve instance_id text: {}", error);

                    Ok(String::from(""))
                }
            }
        }
        Err(error) => {
            log::error!("Could not retrieve the AWS instance_id: {}", error);

            Err(AWSError::InstanceID)
        }
    }
}

pub static AWS_REGION: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::from("")));

pub static AWS_INSTANCE_ID: Lazy<RwLock<String>> = Lazy::new(|| RwLock::new(String::from("")));

pub async fn update_aws_information() {
    let aws_region_result: Result<String, AWSError> = aws_region().await;

    let aws_region: String = match aws_region_result {
        Ok(thing) => thing,
        Err(error) => {
            log::debug!("{}", error);

            String::from("")
        }
    };

    let mut aws_region_write = AWS_REGION
        .write()
        .expect("Could not get a write lock on AWS_REGION.");

    *aws_region_write = aws_region;

    let aws_instance_id_result: Result<String, AWSError> = aws_instance_id().await;

    let aws_instance_id: String = match aws_instance_id_result {
        Ok(thing) => thing,
        Err(error) => {
            log::debug!("{}", error);

            String::from("")
        }
    };

    let mut aws_instance_id_write = AWS_INSTANCE_ID
        .write()
        .expect("Could not get a write lock on AWS_INSTANCE_ID.");

    *aws_instance_id_write = aws_instance_id;
}

pub static GLOBAL_STORAGE: OnceCell<NodeStorage> = OnceCell::new();

pub static NETWORK: Lazy<RwLock<&str>> = Lazy::new(|| RwLock::new("mainnet"));

static THEMELIO_NODE_START_TIME: Lazy<std::time::Instant> = Lazy::new(std::time::Instant::now);

static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

pub static HOSTNAME: Lazy<String> = Lazy::new(|| {
    gethostname::gethostname()
        .into_string()
        .expect("Could not convert hostname into a string.")
});

static HIGHEST_BLOCK: Lazy<IntGauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_int_gauge!(opts!(
        "themelio_node_highest_block",
        "Highest Block",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create HIGHEST_BLOCK IntGauge.")
});

static THEMELIO_NODE_UPTIME_SECONDS: Lazy<IntGauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_int_gauge!(opts!(
        "themelio_node_uptime_seconds",
        "Uptime (Themelio-Node, In Seconds)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create THEMELIO_NODE_UPTIME_SECONDS IntGauge.")
});

static SYSTEM_UPTIME_SECONDS: Lazy<IntGauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_int_gauge!(opts!(
        "themelio_node_system_uptime_seconds",
        "Uptime (System, In Seconds)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create SYSTEM_UPTIME_SECONDS IntGauge.")
});

static MEMORY_TOTAL_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_int_gauge!(opts!(
        "themelio_node_memory_total_bytes",
        "Total Memory (In Bytes)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create MEMORY_TOTAL_BYTES IntGauge.")
});

static MEMORY_FREE_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_int_gauge!(opts!(
        "themelio_node_memory_free_bytes",
        "Free Memory (In Bytes)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create MEMORY_FREE_BYTES IntGauge.")
});

static NETWORK_TRANSMITTED_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_int_gauge!(opts!(
        "themelio_node_network_transmitted_bytes",
        "Network Data Transmitted (In Bytes)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create NETWORK_TRANSMITTED_BYTES IntGauge.")
});

static NETWORK_RECEIVED_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_int_gauge!(opts!(
        "themelio_node_network_received_bytes",
        "Network Data Received (In Bytes)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create NETWORK_RECEIVED_BYTES IntGauge.")
});

static ROOT_FILESYSTEM_TOTAL_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_int_gauge!(opts!(
        "themelio_node_root_filesystem_total_bytes",
        "Root Filesystem Total Space (In Bytes)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create ROOT_FILESYSTEM_TOTAL_BYTES IntGauge.")
});

static ROOT_FILESYSTEM_FREE_BYTES: Lazy<IntGauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_int_gauge!(opts!(
        "themelio_node_root_filesystem_free_bytes",
        "Root Filesystem Free Space (In Bytes)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create ROOT_FILESYSTEM_FREE_BYTES IntGauge.")
});

static CPU_LOAD_USER: Lazy<Gauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_gauge!(opts!(
        "themelio_node_cpu_load_user_percentage",
        "User CPU Load (Percentage)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create CPU_LOAD_USER IntGauge.")
});

static CPU_LOAD_SYSTEM: Lazy<Gauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_gauge!(opts!(
        "themelio_node_cpu_load_system_percentage",
        "System CPU Load (Percentage)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create CPU_LOAD_SYSTEM IntGauge.")
});

static CPU_LOAD_IDLE: Lazy<Gauge> = Lazy::new(|| {
    let aws_instance_id: String = AWS_INSTANCE_ID
        .read()
        .expect("Could not get a read lock on AWS_INSTANCE_ID")
        .clone();

    let aws_region: String = AWS_REGION
        .read()
        .expect("Could not get a read lock on AWS_REGION")
        .clone();

    register_gauge!(opts!(
        "themelio_node_cpu_load_idle_percentage",
        "Idle CPU Load (Percentage)",
        labels! {"hostname" => HOSTNAME.as_str(),
        "network" => *NETWORK.read().expect("Could not get a read lock on NETWORK"),
        "region" => &aws_region,
        "instance_id" => &aws_instance_id}
    ))
    .expect("Could not create CPU_LOAD_IDLE IntGauge.")
});

#[get("/metrics")]
fn metrics() -> Result<String, rweb::http::Error> {
    let text_encoder: TextEncoder = TextEncoder::new();

    let mut encoded_output: Vec<u8> = Vec::new();

    text_encoder
        .encode(&REGISTRY.gather(), &mut encoded_output)
        .expect("A call to .encode() somehow returned an error. This should not happen.");

    let output_string: String = match String::from_utf8(encoded_output) {
        Ok(output) => output,
        Err(error) => {
            log::error!(
                "hostname={} public_ip={} network={} region={} instance_id={} Metrics could not be made into a string from UTF8: {}",
                crate::prometheus::HOSTNAME.as_str(),
                crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
                crate::prometheus::NETWORK.read().expect("Could not get a read lock on NETWORK."),
                AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
                AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
                error
            );

            String::from("There is an error with the metrics")
        }
    };

    Ok(output_string)
}

fn set_highest_block() {
    let storage: &NodeStorage = GLOBAL_STORAGE
        .get()
        .expect("Could not get a lock on GLOBAL_STORAGE");

    let current_block_count: u64 = storage.highest_height().0;

    HIGHEST_BLOCK.set(current_block_count as i64);
}

fn set_themelio_node_uptime() {
    let elapsed_time: time::Duration = THEMELIO_NODE_START_TIME.elapsed();

    THEMELIO_NODE_UPTIME_SECONDS.set(elapsed_time.as_secs() as i64);
}

fn set_system_metrics() {
    let system: PlatformImpl = System::new();

    match system.cpu_load_aggregate() {
        Ok(cpu_load) => {
            thread::sleep(time::Duration::from_secs(1));

            let cpu: CPULoad = cpu_load.done().expect("Could not retrieve CPU load.");

            let cpu_load_user: f32 = cpu.user * 100.0;

            CPU_LOAD_USER.set(cpu_load_user as f64);

            let cpu_load_system: f32 = cpu.system * 100.0;

            CPU_LOAD_SYSTEM.set(cpu_load_system as f64);

            let cpu_load_idle: f32 = cpu.idle * 100.0;

            CPU_LOAD_IDLE.set(cpu_load_idle as f64);
        }
        Err(error) => log::debug!("There was an error retrieving CPU load: {}", error),
    }

    match system.uptime() {
        Ok(uptime) => {
            SYSTEM_UPTIME_SECONDS.set(uptime.as_secs() as i64);
        }
        Err(error) => log::debug!(
            "hostname={} public_ip={} network={} region={} instance_id={} There was an error retrieving system uptime: {}",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            crate::prometheus::NETWORK.read().expect("Could not get a read lock on NETWORK."),
            AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
            AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
            error
        ),
    }

    let default_network_interface: String = default_net::interface::get_default_interface_name()
        .expect("Could not get default network interface name.");

    match system.network_stats(&default_network_interface) {
        Ok(network_statistics) => {
            let transmitted_bytes: u64 = network_statistics.tx_bytes.as_u64();

            let received_bytes: u64 = network_statistics.rx_bytes.as_u64();

            NETWORK_TRANSMITTED_BYTES.set(transmitted_bytes as i64);
            NETWORK_RECEIVED_BYTES.set(received_bytes as i64);
        }
        Err(error) => log::debug!(
            "hostname={} public_ip={} network={} region={} instance_id={} There was an error retrieving network traffic information: {}",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            crate::prometheus::NETWORK.read().expect("Could not get a read lock on NETWORK."),
            AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
            AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
            error
        ),
    }

    let memory: Memory = system.memory().expect("Could not get memory information.");

    MEMORY_TOTAL_BYTES.set(memory.total.as_u64() as i64);

    MEMORY_FREE_BYTES.set(memory.free.as_u64() as i64);

    match system.mounts() {
        Ok(mounts) => {
            mounts.iter().for_each(|mount| {
                if mount.fs_mounted_on == "/" {
                    ROOT_FILESYSTEM_TOTAL_BYTES.set(mount.total.as_u64() as i64);
                    ROOT_FILESYSTEM_FREE_BYTES.set(mount.avail.as_u64() as i64);
                }
            });
        }
        Err(error) => log::debug!(
            "hostname={} public_ip={} network={} region={} instance_id={} There was an error retrieving filesystem information: {}",
            crate::prometheus::HOSTNAME.as_str(),
            crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
            crate::prometheus::NETWORK.read().expect("Could not get a read lock on NETWORK."),
            AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
            AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID"),
            error
        ),
    }
}

pub async fn run_aws_information() {
    loop {
        log::debug!("Starting the update call.");
        let output: Option<()> = update_aws_information()
            .timeout(time::Duration::from_secs(2))
            .await;

        match output {
            Some(_message) => log::debug!("Successfully updated AWS information."),
            None => log::error!("Failed a call to update_aws_information()"),
        }

        let one_minute: time::Duration = time::Duration::from_secs(60);
        Timer::after(one_minute).await;
    }
}

pub async fn prometheus() {
    log::debug!(
        "hostname={} public_ip={} network={} region={} instance_id={} Prometheus metrics listening on http://127.0.0.1:8080",
        crate::prometheus::HOSTNAME.as_str(),
        crate::public_ip_address::PUBLIC_IP_ADDRESS.as_str(),
        crate::prometheus::NETWORK.read().expect("Could not get a read lock on NETWORK."),
        AWS_REGION.read().expect("Could not get a read lock on AWS_REGION"),
        AWS_INSTANCE_ID.read().expect("Could not get a read lock on AWS_INSTANCE_ID")
    );

    REGISTRY
        .register(Box::new(HIGHEST_BLOCK.clone()))
        .expect("Adding HIGHEST_BLOCK to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(THEMELIO_NODE_UPTIME_SECONDS.clone()))
        .expect("Adding THEMELIO_NODE_UPTIME_SECONDS to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(SYSTEM_UPTIME_SECONDS.clone()))
        .expect("Adding SYSTEM_UPTIME_SECONDS to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(MEMORY_TOTAL_BYTES.clone()))
        .expect("Adding MEMORY_TOTAL_BYTES to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(MEMORY_FREE_BYTES.clone()))
        .expect("Adding MEMORY_FREE_BYTES to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(NETWORK_TRANSMITTED_BYTES.clone()))
        .expect("Adding NETWORK_TRANSMITTED_BYTES to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(NETWORK_RECEIVED_BYTES.clone()))
        .expect("Adding NETWORK_RECEIVED_BYTES to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(ROOT_FILESYSTEM_TOTAL_BYTES.clone()))
        .expect("Adding ROOT_FILESYSTEM_TOTAL_BYTES to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(ROOT_FILESYSTEM_FREE_BYTES.clone()))
        .expect("Adding ROOT_FILESYSTEM_FREE_BYTES to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(CPU_LOAD_USER.clone()))
        .expect("Adding CPU_LOAD_USER to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(CPU_LOAD_SYSTEM.clone()))
        .expect("Adding CPU_LOAD_SYSTEM to the prometheus registry failed.");

    REGISTRY
        .register(Box::new(CPU_LOAD_IDLE.clone()))
        .expect("Adding CPU_LOAD_IDLE to the prometheus registry failed.");

    thread::spawn(move || loop {
        let one_second: time::Duration = time::Duration::from_secs(1);

        thread::sleep(one_second);
        set_highest_block();
        set_themelio_node_uptime();
        set_system_metrics();
    });

    serve(metrics()).run(([0, 0, 0, 0], 8080)).await;
}
