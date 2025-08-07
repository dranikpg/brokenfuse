# Simple fs wrapper for python with some small tests

set -o xtrace

if [ ! -d "python-fs" ]; then
  git clone "https://github.com/chaosmail/python-fs.git"
fi

cd python-fs

# Overwrite mount path
echo "Broken fuse is mounted at $BFPATH"
sed -i 's/^DIR = .*/DIR = os.getenv("BFPATH")/' fs/tests/setup.py

pytest
