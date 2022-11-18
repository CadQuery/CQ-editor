#! /bin/bash

BUILD_DIR=$(mktemp -d -p "/tmp/" CQeditor-AppImage-build-XXXXXX)

# Clean up the build directory
cleanup () {
    if [ -d "$BUILD_DIR" ]; then
        rm -rf "$BUILD_DIR"
    fi
}
trap cleanup EXIT

# Save the repo root directory
REPO_ROOT="$(readlink -f $(dirname $(dirname "$0")))"
OLD_CWD="$(readlink -f .)"

export UPD_INFO="gh-releases-zsync|jmright|CQ-editor|latest|CQeditor*x86_64.AppImage.zsync"

echo $REPO_ROOT
echo $OLD_CWD
echo $UPD_INFO

# Set up LinuxDeploy and its conda plugin
wget https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
wget https://raw.githubusercontent.com/TheAssassin/linuxdeploy-plugin-conda/e714783a1ca6fffeeb9dd15bbfce83831bb196f8/linuxdeploy-plugin-conda.sh
chmod +x linuxdeploy*.{sh,AppImage}

# LinuxDeploy plugin config
export CONDA_CHANNELS=conda-forge
export CONDA_PACKAGES=xorg-libxi
export PIP_REQUIREMENTS="."
export PIP_WORKDIR="$REPO_ROOT"
export PIP_VERBOSE=1

mkdir -p AppDir/usr/share/metainfo/
cp "$REPO_ROOT"/*.appdata.xml AppDir/usr/share/metainfo/
cp "$REPO_ROOT"/appimage/cq_logo.png AppDir/
cp "$REPO_ROOT"/appimage/AppRun.sh AppDir/

./linuxdeploy-x86_64.AppImage --appdir AppDir --plugin conda -d "$REPO_ROOT"/appimage/CQ-editor.desktop --custom-apprun AppRun.sh

export VERSION="0.3-dev"

chmod +x CQ-editor*.AppImage*

# Install the AppImage tool
wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x appimagetool*.AppImage
./appimagetool*.AppImage AppDir -u "$UPD_INFO"
