pkg_name=themelio-node-testnet
binary_name=themelio-node
pkg_origin=themelio
pkg_maintainer="Meade Kincke <meade@themelio.org>"
pkg_version="$THEMELIO_NODE_VERSION"
pkg_license=("MPL-2.0")
pkg_full_path="${HAB_CACHE_SRC_PATH}/${binary_name}"
pkg_build_deps=(
  core/sccache
  themelio/rust
)
pkg_deps=(
  core/curl
  core/gcc-libs
  core/nmap
)
pkg_bin_dirs=(bin)
pkg_exports=(
  [port]=port
  [metrics-port]=metrics-port
)
pkg_exposes=(port metrics-port)
pkg_svc_user="root"
pkg_svc_group="$pkg_svc_user"

do_setup_environment() {
  set_buildtime_env SCCACHE_DIR "/usr/local/sccache"
  set_buildtime_env RUSTC_WRAPPER "$(pkg_path_for core/sccache)/bin/sccache"
}

do_verify() {
  return 0
}

do_check() {
  cd "${pkg_full_path}"

  cargo test --locked --verbose
}

do_build() {
  build_line "Creating source directory."
  mkdir -p "${pkg_full_path}/src"

  build_line "Copying lockfile."
  cp /src/Cargo.lock "${pkg_full_path}"

  build_line "Copying manifest."
  cp /src/Cargo.toml "${pkg_full_path}"

  build_line "Copying all source files into package path."
  cp -R /src/src/* "${pkg_full_path}/src/"

  build_line "Entering source directory."
  cd "${pkg_full_path}"

  build_line "Starting Build."
  cargo build --locked --release --features metrics --verbose
}

do_install() {
  local release="${pkg_full_path}/target/release/${binary_name}"
  local target="${pkg_prefix}/target"
  local application_path="${pkg_prefix}/bin/"

  mv  "$release" "$application_path"

  rm -rf "$target"
}