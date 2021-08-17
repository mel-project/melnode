# Metrics

To enable metrics, you will need to include the `metrics` feature:
```
$ cargo build --locked --release --features metrics
```

You can also use the pre-built docker image, which includes the metrics webserver.


## Prometheus Details

The `metrics` feature enables a webserver that runs on port `8080`, with the default prometheus endpoint of `/metrics`.


Example output is as follows:
```
# HELP themelio_node_cpu_load_idle_percentage Idle CPU Load (Percentage)
# TYPE themelio_node_cpu_load_idle_percentage gauge
themelio_node_cpu_load_idle_percentage{hostname="hostname-goes-here",network="mainnet"} 98.23587036132813
# HELP themelio_node_cpu_load_system_percentage System CPU Load (Percentage)
# TYPE themelio_node_cpu_load_system_percentage gauge
themelio_node_cpu_load_system_percentage{hostname="hostname-goes-here",network="mainnet"} 0.009037130512297153
# HELP themelio_node_cpu_load_user_percentage User CPU Load (Percentage)
# TYPE themelio_node_cpu_load_user_percentage gauge
themelio_node_cpu_load_user_percentage{hostname="hostname-goes-here",network="mainnet"} 1.7550925016403198
# HELP themelio_node_highest_block Highest Block
# TYPE themelio_node_highest_block gauge
themelio_node_highest_block{hostname="hostname-goes-here",network="mainnet"} 108518
# HELP themelio_node_memory_free_bytes Free Memory (In Bytes)
# TYPE themelio_node_memory_free_bytes gauge
themelio_node_memory_free_bytes{hostname="hostname-goes-here",network="mainnet"} 19658530816
# HELP themelio_node_memory_total_bytes Total Memory (In Bytes)
# TYPE themelio_node_memory_total_bytes gauge
themelio_node_memory_total_bytes{hostname="hostname-goes-here",network="mainnet"} 33531518976
# HELP themelio_node_network_received_bytes Network Data Received (In Bytes)
# TYPE themelio_node_network_received_bytes gauge
themelio_node_network_received_bytes{hostname="hostname-goes-here",network="mainnet"} 13112110718
# HELP themelio_node_network_transmitted_bytes Network Data Transmitted (In Bytes)
# TYPE themelio_node_network_transmitted_bytes gauge
themelio_node_network_transmitted_bytes{hostname="hostname-goes-here",network="mainnet"} 9058541586
# HELP themelio_node_root_filesystem_free_bytes Root Filesystem Free Space (In Bytes)
# TYPE themelio_node_root_filesystem_free_bytes gauge
themelio_node_root_filesystem_free_bytes{hostname="hostname-goes-here",network="mainnet"} 211633070080
# HELP themelio_node_root_filesystem_total_bytes Root Filesystem Total Space (In Bytes)
# TYPE themelio_node_root_filesystem_total_bytes gauge
themelio_node_root_filesystem_total_bytes{hostname="hostname-goes-here",network="mainnet"} 315993423872
# HELP themelio_node_uptime_seconds Uptime (In Seconds)
# TYPE themelio_node_uptime_seconds gauge
themelio_node_uptime_seconds{hostname="hostname-goes-here",network="mainnet"} 1637959
```