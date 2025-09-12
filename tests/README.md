## Integration testing

Integration is mainly based on stealing tests from open source file system test suites or file system mocking libraries :) Brokenfuse is just ran as a passthrough filesystem.

Define the env var `BFPATH` to point to a mounted broken fuse and run any of the provided shell scripts for testing.

Main testing targets:
* `pyfakefs.sh` - Broad test suite from [pytest-dev/pyfakefs](https://github.com/pytest-dev/pyfakefs)
* `cxx_filesystem.sh` - Simple tests from [C++ filesystem library](https://github.com/gulrak/filesystem) 
* `python_fs.sh` - Simple tests from [Python fs wrapper](https://github.com/chaosmail/python-fs)