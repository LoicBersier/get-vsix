## Tool to download extensions from the Visual Studio Code marketplace

### **disclaimer : This tool is against Visual Studio Code terms of service!**

```
Usage: get-vsix [OPTIONS] <SEARCH>

Arguments:
  <SEARCH>  The name of the extension you are looking for

Options:
  -a, --api <API>                  URL for the Visual Studio Code marketplace
  -l, --limit <LIMIT>              How many extensions to show
  -v, --api-version <API_VERSION>  The version of the api
  -p, --program <PROGRAM>          The program to use to install the extension
  -o, --output <OUTPUT>            Where the file is saved
  -h, --help                       Print help
  -V, --version                    Print version
```

For `-p` option on Windows, you probably want to use `code.bat` as opposed to simply `code` like you would on Linux/Mac

![get-vsix example](doc/get-vsix.gif)
