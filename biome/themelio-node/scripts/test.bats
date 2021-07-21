source "${BATS_TEST_DIRNAME}/../plan.sh"

@test "Version matches" {
  result="$(themelio-node --version | head -1 | awk '{print $2}')"
  [ "$result" = "${pkg_version}" ]
}

@test "Help flag works" {
  run themelio-node --help
  [ $status -eq 0 ]
}

@test "Service is running" {
  result="$(nmap 127.0.0.1 -p 11814 | tail -3 | head -1 | awk '{print $2}')"
  [ "$result" = "open" ]
}