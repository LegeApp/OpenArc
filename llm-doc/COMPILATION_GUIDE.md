# OpenArc GUI Compilation Guide

This guide explains the different ways to compile the OpenArc GUI (DocBrakeGUI) and when to use each method.

## Overview

The OpenArc GUI is a .NET WPF application that can be compiled in different ways depending on your needs:
- **Framework-dependent deployment** (small executable, requires .NET installed)
- **Self-contained deployment** (multiple files with dependencies)
- **Single-file deployment** (one large executable with all dependencies)

## Compilation Methods

### 1. Framework-Dependent Deployment (Default)

This creates a small executable that requires .NET to be installed on the target machine.

#### Command:
```bash
cd DocBrakeGUI
dotnet build -c Release
```

#### Output:
- Small executable (~150KB)
- Multiple DLL files in output directory
- Requires .NET runtime on target machine

#### Use When:
- Target machines have .NET installed
- Disk space is limited
- Fast deployment and updates are needed

### 2. Self-Contained Deployment (Multiple Files)

This creates an application with all necessary runtime files in a single directory.

#### Command:
```bash
cd DocBrakeGUI
dotnet publish -c Release -r win-x64 --self-contained true
```

#### Output:
- ~275 files in output directory
- Total size ~150MB
- No .NET installation required on target machine
- All dependencies included as separate files

#### Use When:
- Target machines don't have .NET installed
- Need to distribute all files together
- Want to avoid conflicts with other .NET applications

### 3. Single-File Deployment (Recommended)

This creates a single large executable that contains everything needed to run the application.

#### Command:
```bash
cd DocBrakeGUI
dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true
```

#### Output:
- Single executable file (~146-150MB)
- No additional files required
- Completely portable
- All dependencies embedded within the executable

#### Use When:
- Want a single distributable file
- Maximum portability is needed
- Don't want to deal with multiple files
- **This is the recommended method for distribution**

## Output Locations

After compilation, executables are placed in:

- **Framework-dependent**: `bin\Release\net8.0\win-x64\`
- **Self-contained**: `bin\Release\net8.0\win-x64\publish\`
- **Single-file**: `bin\Release\net8.0\win-x64\publish\` (or custom path with `-o` flag)

## Advanced Options

### Custom Output Directory
```bash
dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -o "C:\MyOutput\Directory"
```

### Trim Unused Libraries (Smaller Size)
```bash
dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:PublishTrimmed=true
```

> **Note**: Trimming may cause issues with reflection-based code. Test thoroughly.

### ReadyToRun Compilation (Faster Startup)
```bash
dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true -p:PublishReadyToRun=true
```

> **Note**: Increases file size but improves startup time.

## Development vs Production

### Development Builds
- Use `dotnet build` for quick iteration
- Faster compilation times
- Better debugging experience

### Production Builds
- Always use `dotnet publish` for releases
- Optimized for performance
- Includes all necessary dependencies

## Troubleshooting

### Common Issues

1. **"Could not find a part of the path"**
   - Ensure you're in the correct project directory (DocBrakeGUI/)
   - Check that the project file exists (DocBrakeGUI.csproj)

2. **Large file sizes**
   - This is normal for self-contained deployments
   - Single-file deployments are typically 100-150MB due to embedded runtime

3. **Performance concerns**
   - First startup of single-file apps may be slower as libraries are extracted
   - Subsequent startups are normal speed

### Verification Steps

After building, verify your executable:
1. Check file size (should be ~150MB for single-file)
2. Run the executable to ensure it launches properly
3. Test the archive tracking functionality to ensure the database feature works

## Recommended Workflow

For most users wanting to distribute the OpenArc GUI with the archive tracking feature:

1. Navigate to the DocBrakeGUI directory:
   ```bash
   cd D:\misc\arc\openarc\DocBrakeGUI
   ```

2. Build the single-file executable:
   ```bash
   dotnet publish -c Release -r win-x64 --self-contained true -p:PublishSingleFile=true -p:IncludeNativeLibrariesForSelfExtract=true
   ```

3. Find the executable at:
   ```
   bin\Release\net8.0\win-x64\publish\DocBrakeGUI.exe
   ```

This will give you a single ~150MB executable that contains everything needed to run the application with the archive tracking database feature.