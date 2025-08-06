# https://github.com/gulrak/filesystem has a wide range of simple tests
# asserting file system functionality. Run it with broken fuse mounted

set -o xtrace

# Clone gulrak/filesystem
if [ ! -d "filesystem" ]; then
  git clone https://github.com/gulrak/filesystem.git
fi
cd filesystem

# Setup build system
if [ ! -d "filesystem" ]; then
  mkdir build
  cd build
  cmake -DCMAKE_BUILD_TYPE=Debug ..
else
  cd build
fi

# Build
make

# Point to broken fuse
echo "Broken fuse is mounted at $BFPATH"
export TMPDIR=$BFPATH # temp_directory_path reads this

# Run tests
ctest -E "(MixFixture)|(fwd_impl)" --output-on-failure
