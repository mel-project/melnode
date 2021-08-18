pkg_name=themelio-node
pkg_origin=themelio
pkg_maintainer="Meade Kincke <meade@themelio.org>"
pkg_version="0.2.0"
pkg_license=("MPL-2.0")
pkg_full_path="${HAB_CACHE_SRC_PATH}/${pkg_name}"
pkg_build_deps=(core/rust)
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

do_verify() {
  return 0
}

do_check() {
  cd "${pkg_full_path}"

  cargo test --locked --verbose
}

do_build() {
  mkdir -p "${pkg_full_path}/commands"

  mkdir -p "${pkg_full_path}/libs"

  cp -R /src/commands/* "${pkg_full_path}/commands/"

  cp -R /src/libs/* "${pkg_full_path}/libs/"

  cp /src/Cargo.lock "${pkg_full_path}"

  cp /src/Cargo.toml "${pkg_full_path}"

  cd "${pkg_full_path}"

  cargo build --locked --release --features metrics --verbose
}

do_install() {
  local release="${pkg_full_path}/target/release/${pkg_name}"
  local target="${pkg_prefix}/target"
  local application_path="${pkg_prefix}/bin/"

  mv  "$release" "$application_path"

  rm -rf "$target"
}