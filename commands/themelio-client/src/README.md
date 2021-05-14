# Themelio Client CLI Tool

The themelio client CLI tool has two sub commands which provide:
* wallet-shell 
* wallet-utils 

## Wallet Shell 

The wallet shell uses a shell runner which runs the following in a loop:
* Shows a prompt which takes in input
* Executes a command 
* Formats & outputs the results.

The shell has a sub-shell which uses the same structure. Together they allow for operations such as creating & storing wallet data and sending transactions.

It's contents are implemented in the shell folder. It is dependent on the utils and wallet folders.  

### Start interactive shell mode

> cargo run -- --host "94.237.109.44:11814" wallet-shell 

### Open sub-shell for a specific wallet

> themelio-client (v0.1.0) ➜ help

```
Available commands are:
>> create-wallet <wallet-name>
>> open-wallet <wallet-name> <secret>
>> show
>> help
>> exit
>>
```

```
themelio-client (v0.1.0) ➜ open alice e2926ba128937218e12ffa81109fc152d6626f30dddc92641d353a3cc2099b3f0492314a06191bae7eb4d5e4a9d645f23d3cabf8babfd9f698c98f36236d04b9
ERROR: can't parse input command
themelio-client (v0.1.0) ➜ open-wallet alice e2926ba128937218e12ffa81109fc152d6626f30dddc92641d353a3cc2099b3f0492314a06191bae7eb4d5e4a9d645f23d3cabf8babfd9f698c98f36236d04b9
themelio-client (v0.1.0) ➜  (alice) ➜ <you are in sub-shell>
```

> themelio-client (v0.1.0) ➜  (alice) ➜ exit

```
Exiting Themelio Client wallet sub-shell mode
```

> themelio-client (v0.1.0) ➜ exit

```
Exiting Themelio Client wallet-shell mode 
```

## wallet-utils  

The wallet utils executes a command and prints an output in JSON to std out.

This is best suited for automation and testing.

examples:

### Create wallet
> cargo run -- --host "185.70.196.63:11814" wallet-utils create-wallet robert
```
{"name":"robert","address":"t4d4pwewtk0dj7sq1kxam86mv2rv3999qzrcw6g59b1jekr5qykhm0","secret":"5fb33a8b9e2c825b483791761e071aabbf798b28841a975e836383df6d133c76fb7ebefe5a585517d911d4df0cb83ba86bc45d7f328f545f59af53483ee1ff15"}%
```

### Faucet
> cargo run -- --host "94.237.109.44:11814" wallet-utils faucet alice e2926ba128937218e12ffa81109fc152d6626f30dddc92641d353a3cc2099b3f0492314a06191bae7eb4d5e4a9d645f23d3cabf8babfd9f698c98f36236d04b9 1000 TML
