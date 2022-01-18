data "amazon-ami" "debian-bullseye" {
  filters = {
    virtualization-type = "hvm"
    name = "debian-11-amd64-*"
    root-device-type = "ebs"
    architecture = "x86_64"
  }

  most_recent = true
  owners      = ["136693071363"]
}