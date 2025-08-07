# Fake fs emulating real fs for Python by pytest devs

set -o xtrace

if [ ! -d "pyfakefs" ]; then
  git clone https://github.com/pytest-dev/pyfakefs.git
fi

cd pyfakefs

echo "Broken fuse is mounted at $BFPATH"
export TMPDIR=$BDPATH
export TEST_REAL_FS=1

pytest -s pyfakefs/tests/fake_filesystem_test.py pyfakefs/tests/fake_filesystem_vs_real_test.py \
  pyfakefs/tests/fake_os_test.py pyfakefs/tests/fake_open_test.py pytest -s pyfakefs/tests/fake_pathlib_test.py \
