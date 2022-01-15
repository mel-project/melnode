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

// MainNet Section

source "amazon-ebs" "themelio-node-eu-west-2-mainnet" {
  ami_name = "themelio-node"
  instance_type = "t2.micro"
  region = "eu-west-2"
  source_ami = data.amazon-ami.debian-bullseye.id

  launch_block_device_mappings {
    delete_on_termination = true
    device_name = "/dev/xvda"
    volume_size = 50
    volume_type = "gp2"
  }

  ssh_username = "admin"
}

build {
  sources = [
    "source.amazon-ebs.themelio-node-eu-west-2-mainnet",
  ]

  provisioner "ansible" {
    groups = ["themelio_node"]
    playbook_file = "$SCRIPTS_DIRECTORY/ansible-debian-aws/install-mainnet.yml"
#    playbook_file = "./ansible-debian-aws/install-mainnet.yml"
    user = "admin"
    extra_arguments = [
      "--extra-vars", "aws_region=eu-west-2"
    ]
    ansible_env_vars = [
      "ANSIBLE_SSH_ARGS='-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o AddKeysToAgent=no -o HostKeyAlgorithms=+ssh-rsa -o PubkeyAcceptedKeyTypes=+ssh-rsa'",
      "ANSIBLE_HOST_KEY_CHECKING=False"
    ]
  }
}