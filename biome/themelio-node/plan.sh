pkg_name=themelio-node
pkg_origin=themelio
pkg_repository_name="themelio-core"
pkg_maintainer="Meade Kincke <meade@themelio.org>"
pkg_version="0.1.0"
pkg_license=("MPL-2.0")
pkg_full_path="${HAB_CACHE_SRC_PATH}/${pkg_name}-${pkg_version}/${pkg_repository_name}"
pkg_build_deps=(
  core/git
  core/rust
)
pkg_deps=(
  core/curl
  core/gcc-libs
  core/nmap
)
pkg_bin_dirs=(bin)
pkg_exports=(
  [port]=port
)
pkg_exposes=(port)
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
  git clone "https://github.com/themeliolabs/${pkg_repository_name}.git" "${pkg_full_path}"

  cd "${pkg_full_path}"

  cargo build --locked --release --verbose
}

do_install() {
  local release="${pkg_full_path}/target/release/${pkg_name}"
  local target="${pkg_prefix}/target"
  local application_path="${pkg_prefix}/bin/"

  mv  "$release" "$application_path"

  rm -rf "$target"
}