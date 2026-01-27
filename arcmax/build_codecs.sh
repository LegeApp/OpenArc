#!/bin/bash
# Build script for FreeARC codecs using GCC
# This script builds each codec separately and stages them in the codec staging folder

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Project paths
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FREEARC_PATH="${PROJECT_ROOT}/freearc_cpp_lib"
STAGING_PATH="${PROJECT_ROOT}/codec_staging"
BUILD_PATH="${PROJECT_ROOT}/codec_build"

# Compiler settings
export CC=gcc
export CXX=g++
export AR=ar
export RANLIB=ranlib

# Compiler flags
COMMON_FLAGS="-O2 -fPIC -Wall -Wextra -D_WIN32 -DWIN32 -DWIN32_LEAN_AND_MEAN -DNOMINMAX -DNDEBUG -DWINVER=0x0601 -D_WIN32_WINNT=0x0601 -DNOVERSETCONDITIONMASK -D__USE_MINGW_ANSI_STDIO=0"
CXX_FLAGS="${COMMON_FLAGS} -std=c++11"

# Create directories
echo -e "${GREEN}Creating build directories...${NC}"
mkdir -p "${STAGING_PATH}"
mkdir -p "${BUILD_PATH}"

# Function to build a codec
build_codec() {
    local codec_name="$1"
    local source_files=("$2")
    local include_dirs=("$3")
    local output_name="$4"
    
    echo -e "${YELLOW}Building ${codec_name}...${NC}"
    
    # Create codec-specific build directory
    local codec_build_dir="${BUILD_PATH}/${codec_name}"
    mkdir -p "${codec_build_dir}"
    
    # Prepare include flags
    local include_flags=""
    for dir in "${include_dirs[@]}"; do
        include_flags="${include_flags} -I${dir}"
    done
    
    # Compile source files
    local object_files=()
    for src_file in "${source_files[@]}"; do
        local src_name=$(basename "${src_file}")
        local obj_name="${src_name%.*}.o"
        local obj_path="${codec_build_dir}/${obj_name}"
        
        echo "  Compiling ${src_name}..."
        if [[ "${src_file}" == *.cpp ]]; then
            ${CXX} ${CXX_FLAGS} ${include_flags} -c "${src_file}" -o "${obj_path}"
        else
            ${CC} ${COMMON_FLAGS} ${include_flags} -c "${src_file}" -o "${obj_path}"
        fi
        object_files+=("${obj_path}")
    done
    
    # Create static library
    echo "  Creating static library lib${output_name}.a..."
    ${AR} rcs "${STAGING_PATH}/lib${output_name}.a" "${object_files[@]}"
    
    echo -e "${GREEN}✓ ${codec_name} built successfully${NC}"
}

# Function to copy common objects
copy_common_objects() {
    echo -e "${YELLOW}Building common components...${NC}"
    
    # Build common objects
    local common_build_dir="${BUILD_PATH}/common"
    mkdir -p "${common_build_dir}"
    
    # Common.cpp
    echo "  Compiling Common.cpp..."
    ${CXX} ${CXX_FLAGS} -I"${FREEARC_PATH}" -I"${FREEARC_PATH}/Compression" \
        -c "${FREEARC_PATH}/Compression/Common.cpp" -o "${common_build_dir}/Common.o"
    
    # CompressionLibrary.cpp
    echo "  Compiling CompressionLibrary.cpp..."
    ${CXX} ${CXX_FLAGS} -I"${FREEARC_PATH}" -I"${FREEARC_PATH}/Compression" \
        -c "${FREEARC_PATH}/Compression/CompressionLibrary.cpp" -o "${common_build_dir}/CompressionLibrary.o"
    
    # CELS.cpp
    echo "  Compiling CELS.cpp..."
    ${CXX} ${CXX_FLAGS} -I"${FREEARC_PATH}" -I"${FREEARC_PATH}/Compression" \
        -c "${FREEARC_PATH}/Compression/CELS.cpp" -o "${common_build_dir}/CELS.o"
    
    # MultiThreading.cpp
    echo "  Compiling MultiThreading.cpp..."
    ${CXX} ${CXX_FLAGS} -I"${FREEARC_PATH}" -I"${FREEARC_PATH}/Compression" \
        -c "${FREEARC_PATH}/Compression/MultiThreading.cpp" -o "${common_build_dir}/MultiThreading.o"
    
    # Copy to staging
    cp "${common_build_dir}"/*.o "${STAGING_PATH}/"
    
    echo -e "${GREEN}✓ Common components built${NC}"
}

# Build common components first
copy_common_objects

# Build each codec
echo -e "${GREEN}Building codecs...${NC}"

# LZMA2
build_codec "lzma2" \
    "${FREEARC_PATH}/Compression/LZMA2/LZMA2.cpp" \
    "${FREEARC_PATH} ${FREEARC_PATH}/Compression ${FREEARC_PATH}/Compression/LZMA2" \
    "lzma2"

# PPMD
build_codec "ppmd" \
    "${FREEARC_PATH}/Compression/PPMD/C_PPMD.cpp" \
    "${FREEARC_PATH} ${FREEARC_PATH}/Compression ${FREEARC_PATH}/Compression/PPMD" \
    "ppmd"

# Tornado
build_codec "tornado" \
    "${FREEARC_PATH}/Compression/Tornado/C_Tornado.cpp" \
    "${FREEARC_PATH} ${FREEARC_PATH}/Compression ${FREEARC_PATH}/Compression/Tornado" \
    "tornado"

# GRZip
build_codec "grzip" \
    "${FREEARC_PATH}/Compression/GRZip/C_GRZip.cpp" \
    "${FREEARC_PATH} ${FREEARC_PATH}/Compression ${FREEARC_PATH}/Compression/GRZip" \
    "grzip"

# LZP
build_codec "lzp" \
    "${FREEARC_PATH}/Compression/LZP/C_LZP.cpp" \
    "${FREEARC_PATH} ${FREEARC_PATH}/Compression ${FREEARC_PATH}/Compression/LZP" \
    "lzp"

# Delta
build_codec "delta" \
    "${FREEARC_PATH}/Compression/Delta/C_Delta.cpp" \
    "${FREEARC_PATH} ${FREEARC_PATH}/Compression ${FREEARC_PATH}/Compression/Delta" \
    "delta"

# Dict
build_codec "dict" \
    "${FREEARC_PATH}/Compression/Dict/C_Dict.cpp" \
    "${FREEARC_PATH} ${FREEARC_PATH}/Compression ${FREEARC_PATH}/Compression/Dict" \
    "dict"

# MM
build_codec "mm" \
    "${FREEARC_PATH}/Compression/MM/C_MM.cpp" \
    "${FREEARC_PATH} ${FREEARC_PATH}/Compression ${FREEARC_PATH}/Compression/MM" \
    "mm"

# REP
build_codec "rep" \
    "${FREEARC_PATH}/Compression/REP/C_REP.cpp" \
    "${FREEARC_PATH} ${FREEARC_PATH}/Compression ${FREEARC_PATH}/Compression/REP" \
    "rep"

# 4x4
build_codec "4x4" \
    "${FREEARC_PATH}/Compression/4x4/C_4x4.cpp" \
    "${FREEARC_PATH} ${FREEARC_PATH}/Compression ${FREEARC_PATH}/Compression/4x4" \
    "4x4"

# Build the FFI wrapper
echo -e "${YELLOW}Building FFI wrapper...${NC}"
${CXX} ${CXX_FLAGS} \
    -I"${FREEARC_PATH}" \
    -I"${FREEARC_PATH}/Compression" \
    -I"${FREEARC_PATH}/Compression/LZMA2" \
    -I"${FREEARC_PATH}/Compression/PPMD" \
    -I"${FREEARC_PATH}/Compression/Tornado" \
    -I"${FREEARC_PATH}/Compression/GRZip" \
    -I"${FREEARC_PATH}/Compression/LZP" \
    -I"${FREEARC_PATH}/Compression/Delta" \
    -I"${FREEARC_PATH}/Compression/Dict" \
    -I"${FREEARC_PATH}/Compression/MM" \
    -I"${FREEARC_PATH}/Compression/REP" \
    -I"${FREEARC_PATH}/Compression/4x4" \
    -c "${FREEARC_PATH}/freearc_wrapper.cpp" -o "${STAGING_PATH}/freearc_wrapper.o"

echo -e "${GREEN}✓ FFI wrapper built${NC}"

# Create a combined library
echo -e "${YELLOW}Creating combined freearc library...${NC}"
${AR} rcs "${STAGING_PATH}/libfreearc.a" "${STAGING_PATH}"/*.o
echo -e "${GREEN}✓ Combined library created${NC}"

# List built libraries
echo -e "${GREEN}Build complete! Libraries in ${STAGING_PATH}:${NC}"
ls -la "${STAGING_PATH}"/*.a

echo -e "${GREEN}Codec staging directory: ${STAGING_PATH}${NC}"
echo -e "${YELLOW}To use these libraries with Rust, update build.rs to link against: ${STAGING_PATH}/libfreearc.a${NC}"
