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
)
pkg_bin_dirs=(bin)
pkg_exports=(
  [port]=port
)
pkg_exposes=(port)

do_verify() {
  return 0
}

do_check() {
  cd "${pkg_full_path}"

  cargo test --locked --target x86_64-unknown-linux-musl --verbose
}

do_build() {
  git clone "https://github.com/themeliolabs/${pkg_repository_name}.git" "/hab/cache/src/${pkg_name}-${pkg_version}/${pkg_repository_name}"

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