# pjdfstest is a broad test suite for file systems

git clone "https://github.com/pjd/pjdfstest.git"
cd pjdfstest

autoreconf -ifs
./configure
make pjdfstest

echo "Broken fuse is mounted at $BFPATH"
export PJDPATH=$CWD

cd $BFPATH
prove -rv $PJDPATH
