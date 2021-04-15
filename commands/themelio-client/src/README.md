# Themelio Client CLI Tool

The code is structured to support an interactive and non-interactive mode. 

## Command mode 

The non-interactive mode allows a user to execute a single command and exit the binary. This is suited better for automation and testing.

examples:

## Shell mode 

The interactive, or shell, mode allows users to open up a shell and sub-shell to do basic operations such as store wallets and send transactions. 

## Structure

The executor is the primary module used to dispatch high-level non-interactive commands. If the command 'shell' is passed it it will open up a runner in the shell module and beginning executing shell commands. 

The high-level commands are invoked through inputs that are parsed from struct opt. 

Shell mode uses serde_scan to parse and match inputs. 

Where possible, shell mode should depend on command mode.  That is the executor and the io in command mode is the basic dependency. 