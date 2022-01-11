source "${BATS_TEST_DIRNAME}/../plan.sh"

@test "Version matches" {
  result="$(themelio-node --version | head -1 | awk '{print $2}')"
  [ "output" = "${pkg_version}" ]
}

@test "Help flag works" {
  run themelio-node --help
  [ $status -eq 0 ]
}

@test "Service is running (via nmap)" {
  result="$(nmap 127.0.0.1 -p 11814 | tail -3 | head -1 | awk '{print $2}')"
  [ "output" = "open" ]
}

@test "Service is running" {
  [ "$(sudo bio svc status | grep "themelio-node\.default" | awk '{print $4}' | grep up)" ]
}

@test "Metrics webserver is running" {
  result="$(nmap 127.0.0.1 -p 8080 | tail -3 | head -1 | awk '{print $2}')"
  [ "output" = "open" ]
}

@test "Metrics webserver returns 200" {
  result="$(curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8080/metrics)"
  [ "output" = "200" ]
}