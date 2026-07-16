 #################################################################################################################
 #
 #    Copyright (C) 2013-2020 MulticoreWare, Inc
 #
 # This program is free software; you can redistribute it and/or modify
 # it under the terms of the GNU General Public License as published by
 # the Free Software Foundation; either version 2 of the License, or
 # (at your option) any later version.
 #
 # This program is distributed in the hope that it will be useful,
 # but WITHOUT ANY WARRANTY; without even the implied warranty of
 # MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 # GNU General Public License for more details.
 #
 # You should have received a copy of the GNU General Public License
 # along with this program; if not, write to the Free Software
 # Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02111, USA.
 #
 # This program is also available under a commercial proprietary license.
 # For more information, contact us at license @ x265.com
 #
 # Authors: Janani T.E <janani.te@multicorewareinc.com>, Srikanth Kurapati <srikanthkurapati@multicorewareinc.com>
 #
 #################################################################################################################
 # PURPOSE: Identity version control software version display, also read version files to present x265 version.
 #################################################################################################################
 #Default Settings, for user to be vigilant about x265 version being reported during product build.
set(X265_VERSION "unknown")
set(X265_LATEST_TAG "0.0")
set(X265_TAG_DISTANCE "0")

#Find version control software to be used for live and extracted repositories from compressed tarballs
if(CMAKE_VERSION VERSION_LESS "2.8.10")
    find_program(HG_EXECUTABLE hg)
    if(EXISTS "${HG_EXECUTABLE}.bat")
        set(HG_EXECUTABLE "${HG_EXECUTABLE}.bat")
    endif()
    message(STATUS "hg found at ${HG_EXECUTABLE}")
else()
    find_package(Hg QUIET)
endif()
if(HG_EXECUTABLE)
    #Set Version Control binary for source code kind
    if(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/../.hg_archival.txt)
        set(HG_ARCHETYPE "1")
    elseif(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/../.hg)
        set(HG_ARCHETYPE "0")
    endif()
endif(HG_EXECUTABLE)
find_package(Git QUIET) #No restrictions on Git versions used, any versions from 1.8.x to 2.2.x or later should do.
if(GIT_FOUND)
    find_program(GIT_EXECUTABLE git)
    message(STATUS "GIT_EXECUTABLE ${GIT_EXECUTABLE}")
    if(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/../.git)
        set(GIT_ARCHETYPE "0")
    elseif(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/../x265Version.txt)
        set(GIT_ARCHETYPE "1")
    endif()
endif(GIT_FOUND)
if(HG_ARCHETYPE)
    #Read the lines of the archive summary file to extract the version
    message(STATUS "SOURCE CODE IS FROM x265 HG ARCHIVED ZIP OR TAR BALL")
    file(READ ${CMAKE_CURRENT_SOURCE_DIR}/../.hg_archival.txt archive)
    STRING(REGEX REPLACE "\n" ";" archive "${archive}")
    foreach(f ${archive})
        string(FIND "${f}" ": " p