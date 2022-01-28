source "${BATS_TEST_DIRNAME}/../plan.sh"

@test "Version matches" {
  output="$(themelio-node --version | head -1 | awk '{print $2}')"
  [ "$output" = "${pkg_version}" ]
}

@test "Help flag works" {
  run themelio-node --help
  [ $status -eq 0 ]
}

@test "Service is running (via nmap)" {
  output="$(nmap 127.0.0.1 -p 11814 | tail -3 | head -1 | awk '{print $2}')"
  [ "$output" = "open" ]
}

@test "Service is running" {
  [ "$(sudo bio svc status | grep "themelio-node-testnet\.default" | awk '{print $4}' | grep up)" ]
}

@test "Metrics webserver is running" {
  output="$(nmap 127.0.0.1 -p 8080 | tail -3 | head -1 | awk '{print $2}')"
  [ "$output" = "open" ]
}

@test "Metrics webserver returns 200" {
  output="$(curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8080/metrics)"
  [ "$output" = "200" ]
}