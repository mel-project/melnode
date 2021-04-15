# Themelio Client CLI Tool

The code is structured to support an interactive and non-interactive mode. 

## Command mode 

The non-interactive mode allows a user to execute a single command and exit the binary. This is suited better for automation and testing.

examples:

### Create wallet
> cargo run -- --host "94.237.109.44:11814" create-wallet alice
```asm
>> New data:  tigran
>> Address:   t9yey-zvk27-9vr9x-774ce-qckxk-b7gx3-74egz-05xkf-p99ka-f4t06-742g
>> Secret:    e2926ba128937218e12ffa81109fc152d6626f30dddc92641d353a3cc2099b3f0492314a06191bae7eb4d5e4a9d645f23d3cabf8babfd9f698c98f36236d04b9
```

### Faucet
> cargo run -- --host "94.237.109.44:11814" faucet alice e2926ba128937218e12ffa81109fc152d6626f30dddc92641d353a3cc2099b3f0492314a06191bae7eb4d5e4a9d645f23d3cabf8babfd9f698c98f36236d04b9 1000 TML


## Shell mode 

The interactive, or shell, mode allows users to open up a shell and sub-shell to do basic operations such as store wallets and send transactions. 

### Start shell mode
> cargo run -- --host "94.237.109.44:11814" shell

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
Exiting Themelio Client sub-shell
```

> themelio-client (v0.1.0) ➜ exit

```
Exiting Themelio Client shell
```
# Structure

The executor is the primary module used to dispatch high-level non-interactive commands. If the command 'shell' is passed it it will open up a runner in the shell module and beginning executing shell commands. 

The high-level commands are invoked through inputs that are parsed from struct opt. 

Shell mode uses serde_scan to parse and match inputs. 

Where possible, shell mode should depend on command mode.  That is the executor and the io in command mode is the basic dependency. 