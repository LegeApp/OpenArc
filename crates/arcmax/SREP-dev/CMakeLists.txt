cmake_minimum_required(VERSION 3.10)
project(srep_lib VERSION 3.93.0 LANGUAGES CXX)

# C++ standard
set(CMAKE_CXX_STANDARD 11)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# Build type
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

# Compiler flags
set(CMAKE_CXX_FLAGS "-Wall -Wextra")
set(CMAKE_CXX_FLAGS_DEBUG "-g -O0 -DDEBUG")
set(CMAKE_CXX_FLAGS_RELEASE "-O3 -DNDEBUG")

# Options
option(BUILD_SHARED_LIBS "Build shared library instead of static" OFF)
option(BUILD_EXAMPLES "Build example programs" ON)
option(WITH_FREEARC_DEPS "Build with FreeArc dependencies" OFF)
option(ENABLE_ASAN "Enable address sanitizer" OFF)
option(ENABLE_TSAN "Enable thread sanitizer" OFF)

# Library source files
set(SREP_LIB_SOURCES
    srep_lib.cpp
)

set(SREP_LIB_HEADERS
    srep_lib.h
)

# Create library
add_library(srep ${SREP_LIB_SOURCES})

# Include directories
target_include_directories(srep PUBLIC
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>
    $<INSTALL_INTERFACE:include>
)

# Compiler options for library
target_compile_options(srep PRIVATE
    $<$<CXX_COMPILER_ID:GNU>:-march=native>
    $<$<CXX_COMPILER_ID:Clang>:-march=native>
)

# FreeArc dependencies
if(WITH_FREEARC_DEPS)
    target_compile_definitions(srep PRIVATE HAVE_FREEARC_DEPS)
    # Add FreeArc include paths
    # target_include_directories(srep PRIVATE ../FreeArc)
endif()

# Sanitizers
if(ENABLE_ASAN)
    target_compile_options(srep PUBLIC -fsanitize=address -fno-omit-frame-pointer)
    target_link_options(srep PUBLIC -fsanitize=address)
endif()

if(ENABLE_TSAN)
    target_compile_options(srep PUBLIC -fsanitize=thread -fno-omit-frame-pointer)
    target_link_options(srep PUBLIC -fsanitize=thread)
endif()

# Threading support
find_package(Threads REQUIRED)
target_link_libraries(srep PUBLIC Threads::Threads)

# Example programs
if(BUILD_EXAMPLES)
    add_executable(srep_example example.cpp)
    target_link_libraries(srep_example PRIVATE srep)
    
    # Install example
    install(TARGETS srep_example
        RUNTIME DESTINATION bin
    )
endif()

# Installation rules
install(TARGETS srep
    EXPORT srepTargets
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
    RUNTIME DESTINATION bin
    INCLUDES DESTINATION include
)

install(FILES ${SREP_LIB_HEADERS}
    DESTINATION include
)

# Export targets
install(EXPORT srepTargets
    FILE srepTargets.cmake
    NAMESPACE srep::
    DESTINATION lib/cmake/srep
)

# Generate package config
include(CMakePackageConfigHelpers)
write_basic_package_version_file(
    "${CMAKE_CURRENT_BINARY_DIR}/srepConfigVersion.cmake"
    VERSION ${PROJECT_VERSION}
    COMPATIBILITY AnyNewerVersion
)

configure_package_config_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/cmake/srepConfig.cmake.in"
    "${CMAKE_CURRENT_BINARY_DIR}/srepConfig.cmake"
    INSTALL_DESTINATION lib/cmake/srep
)

install(FILES
    "${CMAKE_CURRENT_BINARY_DIR}/srepConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/srepConfigVersion.cmake"
    DESTINATION lib/cmake/srep
)

# CPack configuration
set(CPACK_PACKAGE_NAME "srep")
set(CPACK_PACKAGE_VENDOR "FreeArc")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "SREP - Huge-dictionary LZ77 preprocessor")
set(CPACK_PACKAGE_VERSION "${PROJECT_VERSION}")
set(CPACK_PACKAGE_VERSION_MAJOR "3")
set(CPACK_PACKAGE_VERSION_MINOR "93")
set(CPACK_PACKAGE_VERSION_PATCH "0")
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_CURRENT_SOURCE_DIR}/LICENSE")
set(CPACK_RESOURCE_FILE_README "${CMAKE_CURRENT_SOURCE_DIR}/README.md")

include(CPack)

# Testing
enable_testing()

# Print summary
message(STATUS "")
message(STATUS "SREP Library Configuration:")
message(STATUS "  Version: ${PROJECT_VERSION}")
message(STATUS "  Build type: ${CMAKE_BUILD_TYPE}")
message(STATUS "  Build shared: ${BUILD_SHARED_LIBS}")
message(STATUS "  Build examples: ${BUILD_EXAMPLES}")
message(STATUS "  FreeArc deps: ${WITH_FREEARC_DEPS}")
message(STATUS "  Address sanitizer: ${ENABLE_ASAN}")
message(STATUS "  Thread sanitizer: ${ENABLE_TSAN}")
message(STATUS "")
