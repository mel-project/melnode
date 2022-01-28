# themelio-node

To use our docker image, we have provided a handy docker-compose file.

You will need `docker` and `docker-compose` installed.


To run a full node on the MainNet, simply download the [compose file](docker-compose-mainnet.yml), and run:
```
$ docker-compose up -f docker-compose-mainnet.yml
```

To run a full node on the TestNet, simply download the [compose file](docker-compose-testnet.yml), and run:
```
$ docker-compose up -f docker-compose-testnet.yml
```

There are a number of environment variables that are commented out.
The usage for those are explained in the compose file.