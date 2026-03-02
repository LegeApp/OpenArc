
        /// Tags contained in the ifd namespace
        #[allow(non_upper_case_globals)]
        pub mod ifd {
            #[allow(unused_imports)]
            use super::{IfdFieldDescriptor, IfdValueType, IfdCount, IfdTypeInterpretation, IfdType};
            pub(crate) static ALL: [IfdFieldDescriptor; 235] = [NewSubfileType, SubfileType, ImageWidth, ImageLength, BitsPerSample, Compression, PhotometricInterpretation, Thresholding, CellWidth, CellLength, FillOrder, DocumentName, ImageDescription, Make, Model, StripOffsets, Orientation, SamplesPerPixel, RowsPerStrip, StripByteCounts, MinSampleValue, MaxSampleValue, XResolution, YResolution, PlanarConfiguration, PageName, XPosition, YPosition, FreeOffsets, FreeByteCounts, GrayResponseUnit, GrayResponseCurve, T4Options, T6Options, ResolutionUnit, PageNumber, ColorResponseUnit, TransferFunction, Software, DateTime, Artist, HostComputer, Predictor, WhitePoint, PrimaryChromaticities, ColorMap, HalftoneHints, TileWidth, TileLength, TileOffsets, TileByteCounts, BadFaxLines, CleanFaxData, ConsecutiveBadFaxLines, SubIFDs, InkSet, InkNames, NumberOfInks, DotRange, TargetPrinter, ExtraSamples, SampleFormat, SMinSampleValue, SMaxSampleValue, TransferRange, ClipPath, XClipPathUnits, YClipPathUnits, Indexed, JPEGTables, OPIProxy, Decode, DefaultImageColor, GlobalParametersIFD, ProfileType, FaxProfile, CodingMethods, VersionYear, ModeNumber, T82Options, JPEGProc, JPEGInterchangeFormat, JPEGInterchangeFormatLength, JPEGRestartInterval, JPEGLosslessPredictors, JPEGPointTransforms, JPEGQTables, JPEGDCTables, JPEGACTables, YCbCrCoefficients, YCbCrSubSampling, YCbCrPositioning, ReferenceBlackWhite, StripRowCounts, XMLPacket, USPTOMiscellaneous, ImageID, WangTag1, WangAnnotation, WangTag3, WangTag4, CFARepeatPatternDim, CFAPattern, BatteryLevel, Copyright, ExposureTime, FNumber, ModelPixelScaleTag, AdventScale, AdventRevision, IPTCNAA, INGRPacketData, INGRFlagRegisters, IntergraphMatrix, INGRReserved, ModelTiepointTag, Site, ColorSequence, IT8Header, RasterPadding, BitsPerRunLength, BitsPerExtendedRunLength, ColorTable, ImageColorIndicator, BackgroundColorIndicator, ImageColorValue, BackgroundColorValue, PixelIntensityRange, TransparencyIndicator, ColorCharacterization, HCUsage, KodakIPTC, PixelMagicJBIGOptions, ModelTransformationTag, ImageResourceBlocks, ExifIFD, InterColorProfile, TIFF_FXExtensions, MultiProfiles, SharedData, T88Options, ImageLayer, GeoKeyDirectoryTag, GeoDoubleParamsTag, GeoAsciiParamsTag, ExposureProgram, SpectralSensitivity, GPSInfoIFD, ISOSpeedRatings, OECF, Interlace, TimeZoneOffset, SelfTimerMode, FaxRecvParams, FaxSubAddress, FaxRecvTime, DateTimeOriginal, CompressedBitsPerPixel, ShutterSpeedValue, ApertureValue, BrightnessValue, ExposureBiasValue, MaxApertureValue, SubjectDistance, MeteringMode, LightSource, Flash, FocalLength, FlashEnergy, SpatialFrequencyResponse, Noise, FocalPlaneXResolution, FocalPlaneYResolution, FocalPlaneResolutionUnit, ImageNumber, SecurityClassification, ImageHistory, SubjectLocation, ExposureIndex, TIFFEPStandardID, SensingMethod, CIP3DataFile, CIP3Sheet, CIP3Side, ImageSourceData, GDAL_METADATA, GDAL_NODATA, USPTOOriginalContentType, DNGVersion, DNGBackwardVersion, UniqueCameraModel, LocalizedCameraModel, CFAPlaneColor, CFALayout, LinearizationTable, BlackLevelRepeatDim, BlackLevel, BlackLevelDeltaH, BlackLevelDeltaV, WhiteLevel, DefaultScale, DefaultCropOrigin, DefaultCropSize, ColorMatrix1, ColorMatrix2, CameraCalibration1, CameraCalibration2, ReductionMatrix1, ReductionMatrix2, AnalogBalance, AsShotNeutral, AsShotWhiteXY, BaselineExposure, BaselineNoise, BaselineSharpness, BayerGreenSplit, LinearResponseLimit, CameraSerialNumber, LensInfo, ChromaBlurRadius, AntiAliasStrength, DNGPrivateData, MakerNoteSafety, CalibrationIlluminant1, CalibrationIlluminant2, BestQualityScale, AliasLayerMetadata, TimeCodes, FrameRate, TStop, ReelName, CameraLabel, ProfileName, ProfileToneCurve, ProfileEmbedPolicy, ];
            
        /// Subfile type (new-style)
        ///
        /// The NewSubfileType field contains a bitmask of intents of the IFD.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 36
        pub const NewSubfileType: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "NewSubfileType",
            tag: 254,
            dtype: &[IfdValueType::Long, IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Bitflags { values: &[(0, "ReducedResolution (Reduced-resolution image data)"), (1, "Page (Single page of a multiple page document)"), (2, "Mask (Image mask data)"), (3, "FP (TIFF/IT Final Page)"), (4, "MRC (TIFF-FX Mixed Raster Content)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Subfile type (new-style)",
            long_description: "The NewSubfileType field contains a bitmask of intents of the IFD.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 36",
        }
    ;

    
        /// Subfile type (old-style)
        ///
        /// The SubfileType field contains an enumerated value of intents of the IFD. The SubfileType field is made obsolete by NewSubfileType.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 40
        pub const SubfileType: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubfileType",
            tag: 255,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "FullResolution (Full-resolution image data)"), (2, "ReducedResolution (Reduced-resolution image data)"), (3, "Page (Single page of a multiple page document)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Subfile type (old-style)",
            long_description: "The SubfileType field contains an enumerated value of intents of the IFD. The SubfileType field is made obsolete by NewSubfileType.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 40",
        }
    ;

    
        /// Width of the image in pixels (columns)
        ///
        /// The ImageWidth field is the width of the image, the number of columns in the pel-path direction.
        ///
        /// references:  \
        /// See also<a href="#ImageLength">ImageLength</a>, <a href="#Orientation">Orientation</a>. <a href="#TIFF6">TIFF6</a>, p. 34
        pub const ImageWidth: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageWidth",
            tag: 256,
            dtype: &[IfdValueType::Short, IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Width of the image in pixels (columns)",
            long_description: "The ImageWidth field is the width of the image, the number of columns in the pel-path direction.",
            references: "See also<a href=\"#ImageLength\">ImageLength</a>, <a href=\"#Orientation\">Orientation</a>. <a href=\"#TIFF6\">TIFF6</a>, p. 34",
        }
    ;

    
        /// Length of the image in pixels (rows)
        ///
        /// The ImageLength field is the length of the image, the number of scanlines in the scan direction.
        ///
        /// references:  \
        /// See also<a href="#ImageWidth">ImageWidth</a>, <a href="#Orientation">Orientation</a>. <a href="#TIFF6">TIFF6</a>, p. 34
        pub const ImageLength: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageLength",
            tag: 257,
            dtype: &[IfdValueType::Short, IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Length of the image in pixels (rows)",
            long_description: "The ImageLength field is the length of the image, the number of scanlines in the scan direction.",
            references: "See also<a href=\"#ImageWidth\">ImageWidth</a>, <a href=\"#Orientation\">Orientation</a>. <a href=\"#TIFF6\">TIFF6</a>, p. 34",
        }
    ;

    
        /// Counts of bits per sample
        ///
        /// The BitsPerSample field specifies the precision or number of bits that is used to represent each component of the image in an image sample. This field sometimes contains one value, for each component having the same precision, it should have a value for each component.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 29
        pub const BitsPerSample: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BitsPerSample",
            tag: 258,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Counts of bits per sample",
            long_description: "The BitsPerSample field specifies the precision or number of bits that is used to represent each component of the image in an image sample. This field sometimes contains one value, for each component having the same precision, it should have a value for each component.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 29",
        }
    ;

    
        /// Compression scheme
        ///
        /// The Compression field represents the type of compression used to compress the image data.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 30
        pub const Compression: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Compression",
            tag: 259,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "None (No compression)"), (2, "MH (Modified Huffman compression)"), (3, "Group3 (T.4 compression)"), (4, "Group4 (T.6 (MMR) compression)"), (5, "LZW (LZW (Lempel-Ziv-Welch) compression)"), (6, "OJPEG (JPEG (old-style) compression)"), (7, "JPEG (JPEG (new-style) compression)"), (9, "JBIG (TIFF-FX JBIG (T.82) compression)"), (10, "JBIG_MRC (TIFF-FX JBIG (T.82) MRC (T.43) representation compression)"), (0x7FFE, "NeXT (NeXT 2-bit grey scale compression)"), (0x8003, "Group3_1D_wordalign (Group 3 1-D (MH) compression, word-aligned)"), (0x8005, "Packbits (Macintosh Packbits Run-Length Encoding (RLE) compression)"), (0x8029, "Thunderscan (Thunderscan 4-bit compression)"), (0x807F, "IT8CT_MP_RasterPadding"), (0x8080, "IT8LW_RLE"), (0x8081, "IT8MP_RLE"), (0x8082, "IT8BL_RLE"), (0x808C, "PixarFilm"), (0x808D, "PixarLog"), (0x80B2, "Deflate_experimental (Deflate algorithm compression (experimental value))"), (0x0008, "Deflate (Deflate algorithm compression (standard value))"), (0x80B3, "DCS"), (0x8765, "JBIG_experimental"), (0x8774, "SGILog"), (0x8775, "SGILog24"), (0x8798, "JPEG2000_LEAD"), (0x879B, "JBIG2_TIFF_FX"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Compression scheme",
            long_description: "The Compression field represents the type of compression used to compress the image data.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 30",
        }
    ;

    
        /// Photometric interpretation
        ///
        /// The PhotometricInterpretation field describes the colorspace the image samples represent.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 37
        pub const PhotometricInterpretation: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "PhotometricInterpretation",
            tag: 262,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "WhiteIsZero (White is zero)"), (1, "BlackIsZero (Black is zero)"), (2, "RGB (RGB (Red, Green, Blue))"), (3, "PaletteColor (Palette color)"), (4, "TransparencyMask (Transparency mask)"), (5, "Separated (Separation)"), (6, "YCbCr (YCbCr)"), (8, "CIELab (CIE L*a*b*)"), (9, "ICCLab (ICC L*a*b*)"), (10, "ITULab (ITU-T Facsimile L*a*b*)"), (32844, "LogL (Log luminance)"), (32845, "LogLUV (Log luminance and chrominance)"), (32803, "CFA (Color filter array)"), (34892, "LinearRaw (DNG Linear Raw)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Photometric interpretation",
            long_description: "The PhotometricInterpretation field describes the colorspace the image samples represent.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 37",
        }
    ;

    
        /// Halftone/dithering algorithm
        ///
        /// The Thresholding field contains the type of the halftoning/dithering algorithm used.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 41
        pub const Thresholding: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Thresholding",
            tag: 263,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "NoDither"), (2, "OrderedDither"), (3, "RandomizedProcess"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Halftone/dithering algorithm",
            long_description: "The Thresholding field contains the type of the halftoning/dithering algorithm used.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 41",
        }
    ;

    
        /// Width of the halftone or dither cell
        ///
        /// The CellWidth fields contains the width of the halftone cell. This field should only be present if Thresholding==OrderedDither.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 29
        pub const CellWidth: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CellWidth",
            tag: 264,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Width of the halftone or dither cell",
            long_description: "The CellWidth fields contains the width of the halftone cell. This field should only be present if Thresholding==OrderedDither.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 29",
        }
    ;

    
        /// Length of the halftone or dither cell
        ///
        /// The CellLength field contains the length of the halftone cell. This field should only be present if Thresholding==OrderedDither.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 29
        pub const CellLength: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CellLength",
            tag: 265,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Length of the halftone or dither cell",
            long_description: "The CellLength field contains the length of the halftone cell. This field should only be present if Thresholding==OrderedDither.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 29",
        }
    ;

    
        /// The bit order within coded image data
        ///
        /// The FillOrder field describes the bit order of the compressed image data. This is almost always msb-to-lsb.  The native form of Group 3 facsimile devices is lsb-to-msb.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 32
        pub const FillOrder: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FillOrder",
            tag: 266,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "MSBtoLSB (most-significant-bit to least-significant-bit)"), (2, "LSBtoMSB (least-significant-bit to most-significant-bit)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "The bit order within coded image data",
            long_description: "The FillOrder field describes the bit order of the compressed image data. This is almost always msb-to-lsb.  The native form of Group 3 facsimile devices is lsb-to-msb.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 32",
        }
    ;

    
        /// Document name
        ///
        /// The DocumentName fields contains the name of the document.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 12:  Document Storage and Retrieval", p. 55
        pub const DocumentName: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DocumentName",
            tag: 269,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Document name",
            long_description: "The DocumentName fields contains the name of the document.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 12:  Document Storage and Retrieval\", p. 55",
        }
    ;

    
        /// Image description
        ///
        /// The ImageDescription fields contains text describing the image.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>
        pub const ImageDescription: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageDescription",
            tag: 270,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Image description",
            long_description: "The ImageDescription fields contains text describing the image.",
            references: "<a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Input device make
        ///
        /// The Make field defines the manufacturer of the input scanner/camera that digitized the image.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>
        pub const Make: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Make",
            tag: 271,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Input device make",
            long_description: "The Make field defines the manufacturer of the input scanner/camera that digitized the image.",
            references: "<a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Input device model
        ///
        /// The Model field defines the model of the input scanner/camera that digitized the image.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>
        pub const Model: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Model",
            tag: 272,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Input device model",
            long_description: "The Model field defines the model of the input scanner/camera that digitized the image.",
            references: "<a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Offsets to strip data
        ///
        /// The StripOffsets field contains a value for each strip of the image that is the offset to the beginning of the strip data. The strips per image is determined from the image length and the rows per strip.  See ImageLength, RowsPerStrip. If the image has PlanarConfiguration==Planar then there is an offset for each sample for each strip of the image, with the offsets pointing to in order the strips for each separated component.
        ///
        /// references:  \
        /// See also<a href="#StripByteCounts">StripByteCounts</a>, <a href="#RowsPerStrip">RowsPerStrip</a>. <a href="#TIFF6">TIFF6</a>, p. 40
        pub const StripOffsets: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "StripOffsets",
            tag: 273,
            dtype: &[IfdValueType::Long, IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Offsets { lengths: &StripByteCounts },
            count: IfdCount::N,
            description: "Offsets to strip data",
            long_description: "The StripOffsets field contains a value for each strip of the image that is the offset to the beginning of the strip data. The strips per image is determined from the image length and the rows per strip.  See ImageLength, RowsPerStrip. If the image has PlanarConfiguration==Planar then there is an offset for each sample for each strip of the image, with the offsets pointing to in order the strips for each separated component.",
            references: "See also<a href=\"#StripByteCounts\">StripByteCounts</a>, <a href=\"#RowsPerStrip\">RowsPerStrip</a>. <a href=\"#TIFF6\">TIFF6</a>, p. 40",
        }
    ;

    
        /// Orientation of image
        ///
        /// The Orientation field describes how the output image is to be interpreted by rotating or flipping the coordinate origin.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>
        pub const Orientation: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Orientation",
            tag: 274,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "RowTopColumnLeft"), (2, "RowTopColumnRight"), (3, "RowBottomColumnRight"), (4, "RowBottomColumnLeft"), (5, "RowLeftColumnTop"), (6, "RowRightColumnTop"), (7, "RowRightColumnBottom"), (8, "RowLeftColumnBottom"), (9, "Unknown"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Orientation of image",
            long_description: "The Orientation field describes how the output image is to be interpreted by rotating or flipping the coordinate origin.",
            references: "<a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Count of samples per pixel
        ///
        /// The SamplesPerPixel field is how many image or data component samples there are for each pixel. Each pixel has the same number of samples, except in the case of subsampled YCbCr and ITU/Facsimile L*a*b* per TIFF 6.0 and TIFF-FX, in which case this value reflects the number of components. Each sample represents an channel or component given the photometric interpretation, unless extra samples are present.  See ExtraSamples. The image may have multiple components but be a palettized image, in which case SamplesPerPixel would only reflect one sample for the color map entry.  See Indexed, PhotometricInterpretation==PaletteColor.
        ///
        /// references:  \
        /// See also<a href="#ExtraSamples">ExtraSamples</a>, <a href="#Indexed">Indexed</a>, <a href="#PhotometricInterpretation">PhotometricInterpretation</a>. <a href="#TIFF6">TIFF6</a>, p. 39
        pub const SamplesPerPixel: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SamplesPerPixel",
            tag: 277,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Count of samples per pixel",
            long_description: "The SamplesPerPixel field is how many image or data component samples there are for each pixel. Each pixel has the same number of samples, except in the case of subsampled YCbCr and ITU/Facsimile L*a*b* per TIFF 6.0 and TIFF-FX, in which case this value reflects the number of components. Each sample represents an channel or component given the photometric interpretation, unless extra samples are present.  See ExtraSamples. The image may have multiple components but be a palettized image, in which case SamplesPerPixel would only reflect one sample for the color map entry.  See Indexed, PhotometricInterpretation==PaletteColor.",
            references: "See also<a href=\"#ExtraSamples\">ExtraSamples</a>, <a href=\"#Indexed\">Indexed</a>, <a href=\"#PhotometricInterpretation\">PhotometricInterpretation</a>. <a href=\"#TIFF6\">TIFF6</a>, p. 39",
        }
    ;

    
        /// Rows per strip
        ///
        /// The RowsPerStrip field contains the number of rows (scanlines) per strip. It is used to determine the strips per image. A value of 0xFFFFFFFF, or the maximum value of the data type, implies that the entire image is one strip.
        ///
        /// references:  \
        /// See also<a href="#StripOffsets">StripOffsets</a>, <a href="#StripByteCounts">StripByteCounts</a>. <a href="#TIFF6">TIFF6</a>, p. 39
        pub const RowsPerStrip: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "RowsPerStrip",
            tag: 278,
            dtype: &[IfdValueType::Long, IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Rows per strip",
            long_description: "The RowsPerStrip field contains the number of rows (scanlines) per strip. It is used to determine the strips per image. A value of 0xFFFFFFFF, or the maximum value of the data type, implies that the entire image is one strip.",
            references: "See also<a href=\"#StripOffsets\">StripOffsets</a>, <a href=\"#StripByteCounts\">StripByteCounts</a>. <a href=\"#TIFF6\">TIFF6</a>, p. 39",
        }
    ;

    
        /// Byte counts of strip data
        ///
        /// The StripByteCounts field contains a value for each strip of the image that is the byte count of the strip. The strips per image is determined from the image length and the rows per strip.  See ImageLength, RowsPerStrip. If the image has PlanarConfiguration==Planar then there is an offset for each sample for each strip of the image, with the offsets pointing to in order the strips for each separated component.
        ///
        /// references:  \
        /// See also<a href="#StripOffsets">StripOffsets</a>, <a href="#RowsPerStrip">RowsPerStrip</a>. <a href="#TIFF6">TIFF6</a>, p. 40
        pub const StripByteCounts: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "StripByteCounts",
            tag: 279,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Lengths,
            count: IfdCount::N,
            description: "Byte counts of strip data",
            long_description: "The StripByteCounts field contains a value for each strip of the image that is the byte count of the strip. The strips per image is determined from the image length and the rows per strip.  See ImageLength, RowsPerStrip. If the image has PlanarConfiguration==Planar then there is an offset for each sample for each strip of the image, with the offsets pointing to in order the strips for each separated component.",
            references: "See also<a href=\"#StripOffsets\">StripOffsets</a>, <a href=\"#RowsPerStrip\">RowsPerStrip</a>. <a href=\"#TIFF6\">TIFF6</a>, p. 40",
        }
    ;

    
        /// Minimum sample value
        ///
        /// The MinSampleValue field identifies the least sample value for each sample, from the range of values possible given the bits per sample of the sample as an unsigned integer. This is for use only for statistical purposes and not footroom of the sample.
        ///
        /// references:  \
        /// See also<a href="#MaxSampleValue">MaxSampleValue</a>. <a href="#TIFF6">TIFF6</a>, p. 36
        pub const MinSampleValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "MinSampleValue",
            tag: 280,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Minimum sample value",
            long_description: "The MinSampleValue field identifies the least sample value for each sample, from the range of values possible given the bits per sample of the sample as an unsigned integer. This is for use only for statistical purposes and not footroom of the sample.",
            references: "See also<a href=\"#MaxSampleValue\">MaxSampleValue</a>. <a href=\"#TIFF6\">TIFF6</a>, p. 36",
        }
    ;

    
        /// Maximum sample value
        ///
        /// The MaxSamplesValue field identifies the greatest sample value for each sample, from the range of values possible given the bits per sample of the sample as an unsigned integer. This is for use only for statistical purposes and not headroom of the sample.
        ///
        /// references:  \
        /// See also<a href="#MinSampleValue">MinSampleValue</a>. <a href="#TIFF6">TIFF6</a>, p. 36
        pub const MaxSampleValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "MaxSampleValue",
            tag: 281,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Maximum sample value",
            long_description: "The MaxSamplesValue field identifies the greatest sample value for each sample, from the range of values possible given the bits per sample of the sample as an unsigned integer. This is for use only for statistical purposes and not headroom of the sample.",
            references: "See also<a href=\"#MinSampleValue\">MinSampleValue</a>. <a href=\"#TIFF6\">TIFF6</a>, p. 36",
        }
    ;

    
        /// Horizontal resolution
        ///
        /// The XResolution contains the pixels per resolution unit in the horizontal direction, before orientation.
        ///
        /// references:  \
        /// See also<a href="#YResolution">YResolution</a>, <a href="#ResolutionUnit">ResolutionUnit</a>, <a href="#Orientation">Orientation</a>. <a href="#TIFF6">TIFF6</a>
        pub const XResolution: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "XResolution",
            tag: 282,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Horizontal resolution",
            long_description: "The XResolution contains the pixels per resolution unit in the horizontal direction, before orientation.",
            references: "See also<a href=\"#YResolution\">YResolution</a>, <a href=\"#ResolutionUnit\">ResolutionUnit</a>, <a href=\"#Orientation\">Orientation</a>. <a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Vertical resolution
        ///
        /// The YResolution contains the pixels per resolution unit in the vertical direction, before orientation.
        ///
        /// references:  \
        /// See also<a href="#XResolution">XResolution</a>, <a href="#ResolutionUnit">ResolutionUnit</a>, <a href="#Orientation">Orientation</a>. <a href="#TIFF6">TIFF6</a>
        pub const YResolution: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "YResolution",
            tag: 283,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Vertical resolution",
            long_description: "The YResolution contains the pixels per resolution unit in the vertical direction, before orientation.",
            references: "See also<a href=\"#XResolution\">XResolution</a>, <a href=\"#ResolutionUnit\">ResolutionUnit</a>, <a href=\"#Orientation\">Orientation</a>. <a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Configuration of data interleaving
        ///
        /// The PlanarConfiguration field describes how the data is interleaved, for example pixel interleaved, scanline interleaved, or component interleaved.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 38
        pub const PlanarConfiguration: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "PlanarConfiguration",
            tag: 284,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "Chunky (Chunky (component interleaved) sample organization)"), (2, "Planar (Planar (channel interleaved) sample organization)"), (0x8000, "Line (Line (line interleaved) sample organization (IT8))"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Configuration of data interleaving",
            long_description: "The PlanarConfiguration field describes how the data is interleaved, for example pixel interleaved, scanline interleaved, or component interleaved.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 38",
        }
    ;

    
        /// Page name
        ///
        /// The PageName field contains the name of a page within a multiple-page document, for example a logical page number.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 12:  Document Storage and Retrieval", p. 55
        pub const PageName: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "PageName",
            tag: 285,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Page name",
            long_description: "The PageName field contains the name of a page within a multiple-page document, for example a logical page number.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 12:  Document Storage and Retrieval\", p. 55",
        }
    ;

    
        /// Horizontal positional offset
        ///
        /// The XPosition field defines the horizontal offset from the edge of an output page of the image data. The XPosition field is used in TIFF-FX MRC to define the offset of image elements in MRC.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 12:  Document Storage and Retrieval", p. 55///  <a href="#TIFFFX">TIFFFX</a>
        pub const XPosition: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "XPosition",
            tag: 286,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Horizontal positional offset",
            long_description: "The XPosition field defines the horizontal offset from the edge of an output page of the image data. The XPosition field is used in TIFF-FX MRC to define the offset of image elements in MRC.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 12:  Document Storage and Retrieval\", p. 55\n <a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// Vertical positional offset
        ///
        /// The YPosition field defines the vertical offset from the edge of an output page of the image data. The YPosition field is used in TIFF-FX MRC to define the offset of image elements in MRC.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 12:  Document Storage and Retrieval", p. 56///  <a href="#TIFFFX">TIFFFX</a>
        pub const YPosition: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "YPosition",
            tag: 287,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Vertical positional offset",
            long_description: "The YPosition field defines the vertical offset from the edge of an output page of the image data. The YPosition field is used in TIFF-FX MRC to define the offset of image elements in MRC.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 12:  Document Storage and Retrieval\", p. 56\n <a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// Offsets to unused file areas
        ///
        /// The FreeOffsets field contains an array of offsets to unused regions within the file.
        ///
        /// references:  \
        /// 
        pub const FreeOffsets: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FreeOffsets",
            tag: 288,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Offsets { lengths: &FreeByteCounts },
            count: IfdCount::N,
            description: "Offsets to unused file areas",
            long_description: "The FreeOffsets field contains an array of offsets to unused regions within the file.",
            references: "",
        }
    ;

    
        /// Byte counts of unused file areas
        ///
        /// The FreeOffsets field contains an array of byte counts of unused regions within the file.
        ///
        /// references:  \
        /// 
        pub const FreeByteCounts: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FreeByteCounts",
            tag: 289,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Lengths,
            count: IfdCount::N,
            description: "Byte counts of unused file areas",
            long_description: "The FreeOffsets field contains an array of byte counts of unused regions within the file.",
            references: "",
        }
    ;

    
        /// Grayscale response curve units
        ///
        /// The GrayResponseUnit field contains a value describing the multiplier of the GrayResponseCurve values.  Each GrayResponseCurve value is multiplied by the value of the GrayResponseUnit, eg 1/10, 1/100, 1/1000, etcetera.
        ///
        /// references:  \
        /// See also<a href="#GrayResponseCurve">GrayResponseCurve</a>. <a href="#TIFF6">TIFF6</a>, p. 33
        pub const GrayResponseUnit: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GrayResponseUnit",
            tag: 290,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "Tenths"), (2, "Hundredths"), (3, "Thousandths"), (4, "TenThousandths"), (5, "HundredThousandths"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Grayscale response curve units",
            long_description: "The GrayResponseUnit field contains a value describing the multiplier of the GrayResponseCurve values.  Each GrayResponseCurve value is multiplied by the value of the GrayResponseUnit, eg 1/10, 1/100, 1/1000, etcetera.",
            references: "See also<a href=\"#GrayResponseCurve\">GrayResponseCurve</a>. <a href=\"#TIFF6\">TIFF6</a>, p. 33",
        }
    ;

    
        /// Grayscale response curve
        ///
        /// The GrayResponseCurve contains a value for each of the values of BitsPerSample many bits that describes the optical density of a pixel having that value. The values of the GrayResponseCurve are each to be multiplied by the factor described in the GrayResponseUnits field.
        ///
        /// references:  \
        /// See also<a href="#GrayResponseUnit">GrayResponseUnit</a>, <a href="#ColorResponseCurves">ColorResponseCurves</a>, <a href="#TransferFunction">TransferFunction</a>. <a href="#TIFF6">TIFF6</a>, p. 33
        pub const GrayResponseCurve: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GrayResponseCurve",
            tag: 291,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Grayscale response curve",
            long_description: "The GrayResponseCurve contains a value for each of the values of BitsPerSample many bits that describes the optical density of a pixel having that value. The values of the GrayResponseCurve are each to be multiplied by the factor described in the GrayResponseUnits field.",
            references: "See also<a href=\"#GrayResponseUnit\">GrayResponseUnit</a>, <a href=\"#ColorResponseCurves\">ColorResponseCurves</a>, <a href=\"#TransferFunction\">TransferFunction</a>. <a href=\"#TIFF6\">TIFF6</a>, p. 33",
        }
    ;

    
        /// Group 3 Fax options
        ///
        /// The T4Options field contains options of the ITU-T T.4 (CCITT Group 3 Facsmile) coding.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 11:  CCITT Bilevel Encodings", p. 51
        pub const T4Options: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "T4Options",
            tag: 292,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Bitflags { values: &[(0, "2D"), (1, "UncompressedMode"), (2, "EOLByteAlign"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Group 3 Fax options",
            long_description: "The T4Options field contains options of the ITU-T T.4 (CCITT Group 3 Facsmile) coding.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 11:  CCITT Bilevel Encodings\", p. 51",
        }
    ;

    
        /// Group 4 Fax options
        ///
        /// The T6Options field contains options of the ITU-T T.6 (CCITT Group 4 Facsmile) coding.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 11:  CCITT Bilevel Encodings", p. 52
        pub const T6Options: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "T6Options",
            tag: 293,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Bitflags { values: &[(1, "UncompressedMode"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Group 4 Fax options",
            long_description: "The T6Options field contains options of the ITU-T T.6 (CCITT Group 4 Facsmile) coding.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 11:  CCITT Bilevel Encodings\", p. 52",
        }
    ;

    
        /// Resolution units
        ///
        /// The ResolutionUnit field contains whether the resolution unit is inch, centimeter, or unknown/unspecified.
        ///
        /// references:  \
        /// See also<a href="#XResolution">XResolution</a>, <a href="#YResolution">YResolution</a>. <a href="#TIFF6">TIFF6</a>
        pub const ResolutionUnit: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ResolutionUnit",
            tag: 296,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "Unitless (Units not specified)"), (2, "Inch (Units in inches)"), (3, "Centimeter (Units in centimeters)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Resolution units",
            long_description: "The ResolutionUnit field contains whether the resolution unit is inch, centimeter, or unknown/unspecified.",
            references: "See also<a href=\"#XResolution\">XResolution</a>, <a href=\"#YResolution\">YResolution</a>. <a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Page number
        ///
        /// The PageNumber field contains two values, the first being the page index for this page and the second being the number of pages or zero for unknown.  Some broken applications reverse these values.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 12:  Document Storage and Retrieval", p. 55
        pub const PageNumber: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "PageNumber",
            tag: 297,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "Page number",
            long_description: "The PageNumber field contains two values, the first being the page index for this page and the second being the number of pages or zero for unknown.  Some broken applications reverse these values.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 12:  Document Storage and Retrieval\", p. 55",
        }
    ;

    
        /// Color response curve units
        ///
        /// The ColorResponseUnit field contains a value describing the multiplier of the ColorResponseCurves values.  Each of the ColorResponseCurves' values is multiplied by the value of the ColorResponseUnit, eg 1/10, 1/100, 1/1000, etcetera.
        ///
        /// references:  \
        /// See also<a href="#ColorResponseCurves">ColorResponseCurves</a>. <a href="#TIFF4">TIFF4</a>
        pub const ColorResponseUnit: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ColorResponseUnit",
            tag: 300,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "Tenths"), (2, "Hundredths"), (3, "Thousandths"), (4, "TenThousandths"), (5, "HundredThousandths"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Color response curve units",
            long_description: "The ColorResponseUnit field contains a value describing the multiplier of the ColorResponseCurves values.  Each of the ColorResponseCurves' values is multiplied by the value of the ColorResponseUnit, eg 1/10, 1/100, 1/1000, etcetera.",
            references: "See also<a href=\"#ColorResponseCurves\">ColorResponseCurves</a>. <a href=\"#TIFF4\">TIFF4</a>",
        }
    ;

    
        /// Transfer function
        ///
        /// The TransferFunction field contains a value for each of the possible values of the pixel for each component or all components. The TransferFunction tag coincides with the ColorResponseCurves tag, if the ColorResponseUnits field exists then TransferFunction should be treated as ColorResponseCurves.
        ///
        /// references:  \
        /// See also<a href="#TransferRange">TransferRange</a>, <a href="#ReferenceBlackWhite">ReferenceBlackWhite</a>. <a href="#TIFF6">TIFF6</a>, "Section 20:  RGB Image Colorimetry", p. 84
        pub const TransferFunction: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TransferFunction",
            tag: 301,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Transfer function",
            long_description: "The TransferFunction field contains a value for each of the possible values of the pixel for each component or all components. The TransferFunction tag coincides with the ColorResponseCurves tag, if the ColorResponseUnits field exists then TransferFunction should be treated as ColorResponseCurves.",
            references: "See also<a href=\"#TransferRange\">TransferRange</a>, <a href=\"#ReferenceBlackWhite\">ReferenceBlackWhite</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 20:  RGB Image Colorimetry\", p. 84",
        }
    ;

    
        /// Software version
        ///
        /// The Software field defines the software product and version that generated the image file.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>
        pub const Software: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Software",
            tag: 305,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Software version",
            long_description: "The Software field defines the software product and version that generated the image file.",
            references: "<a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Date and time of image creation
        ///
        /// The DateTime contains an ASCII string of 20 characters, with the ending binary zero. The ASCII string encodes a date and time as "YYYY:MM:DD HH:MM:SS".
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>
        pub const DateTime: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DateTime",
            tag: 306,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(20),
            description: "Date and time of image creation",
            long_description: "The DateTime contains an ASCII string of 20 characters, with the ending binary zero. The ASCII string encodes a date and time as \"YYYY:MM:DD HH:MM:SS\".",
            references: "<a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Person who created the image
        ///
        /// The Artist field contains the artist/author of the image. This field is sometimes used to hold copyright information. This field sometimes contains two null-terminated strings, with the second being copyright information.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>
        pub const Artist: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Artist",
            tag: 315,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Person who created the image",
            long_description: "The Artist field contains the artist/author of the image. This field is sometimes used to hold copyright information. This field sometimes contains two null-terminated strings, with the second being copyright information.",
            references: "<a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Host computer
        ///
        /// The HostComputer field contains the name of the computer which created the image file.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>
        pub const HostComputer: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "HostComputer",
            tag: 316,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Host computer",
            long_description: "The HostComputer field contains the name of the computer which created the image file.",
            references: "<a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// Differencing predictor
        ///
        /// The Predictor field describes the differencing predictor method used. It is used with LZW and perhaps also Deflate compression schemes.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 14: Differencing Predictor", p. 64
        pub const Predictor: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Predictor",
            tag: 317,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "NoPrediction (No prediction sheme)"), (2, "HorizontalDifferencing (Horizontal differencing prediction scheme)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Differencing predictor",
            long_description: "The Predictor field describes the differencing predictor method used. It is used with LZW and perhaps also Deflate compression schemes.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 14: Differencing Predictor\", p. 64",
        }
    ;

    
        /// Chromaticity of white point
        ///
        /// The WhitePoint field contains two values that describe the CIE xy components of the chromaticity of the white point, where the primaries (channels) have their reference white values.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 20:  RGB Image Colorimetry", p. 83
        pub const WhitePoint: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "WhitePoint",
            tag: 318,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "Chromaticity of white point",
            long_description: "The WhitePoint field contains two values that describe the CIE xy components of the chromaticity of the white point, where the primaries (channels) have their reference white values.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 20:  RGB Image Colorimetry\", p. 83",
        }
    ;

    
        /// Chromaticities of primaries
        ///
        /// The PrimaryChromaticities field contains six values that described the CIE xy components of the chromaticity of the three primaries (channels) at their reference white values, where the other primaries are at their reference black values.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 20:  RGB Image Colorimetry", p. 83
        pub const PrimaryChromaticities: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "PrimaryChromaticities",
            tag: 319,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(6),
            description: "Chromaticities of primaries",
            long_description: "The PrimaryChromaticities field contains six values that described the CIE xy components of the chromaticity of the three primaries (channels) at their reference white values, where the other primaries are at their reference black values.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 20:  RGB Image Colorimetry\", p. 83",
        }
    ;

    
        /// Color map / palette
        ///
        /// The ColorMap field contains a 16-bit value for each component. The values are component interleaved, for example for an RGB images the values of the colormap are stored RRR...GGG...BBB.... The colormap may contain a map for three or four component images, when either PhotometricInterpretation==PaletteColor or Indexed==Indexed.
        ///
        /// references:  \
        /// See also<a href="#PhotometricInterpretation">PhotometricInterpretation</a>, <a href="#Indexed">Indexed</a>. <a href="#TIFF6">TIFF6</a>, "Section 5:  Palette-color Images", p. 23
        pub const ColorMap: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ColorMap",
            tag: 320,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Color map / palette",
            long_description: "The ColorMap field contains a 16-bit value for each component. The values are component interleaved, for example for an RGB images the values of the colormap are stored RRR...GGG...BBB.... The colormap may contain a map for three or four component images, when either PhotometricInterpretation==PaletteColor or Indexed==Indexed.",
            references: "See also<a href=\"#PhotometricInterpretation\">PhotometricInterpretation</a>, <a href=\"#Indexed\">Indexed</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 5:  Palette-color Images\", p. 23",
        }
    ;

    
        /// Halftone hints
        ///
        /// The HalftoneHints field contains range extents for the halftone function of values to retain tonal value.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 17:  HalftoneHints", p. 72
        pub const HalftoneHints: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "HalftoneHints",
            tag: 321,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "Halftone hints",
            long_description: "The HalftoneHints field contains range extents for the halftone function of values to retain tonal value.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 17:  HalftoneHints\", p. 72",
        }
    ;

    
        /// Tile width
        ///
        /// The TileWidth field contains the width of the tiles of the tiled image. TileWidth must be a multiple of 16.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 15:  Tiled Images", p. 66
        pub const TileWidth: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TileWidth",
            tag: 322,
            dtype: &[IfdValueType::Short, IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Tile width",
            long_description: "The TileWidth field contains the width of the tiles of the tiled image. TileWidth must be a multiple of 16.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 15:  Tiled Images\", p. 66",
        }
    ;

    
        /// Tile length
        ///
        /// The TileLength field contains the length of the tiles in the tiled image. TileLength must be a multiple of 16.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 15:  Tiled Images", p. 66
        pub const TileLength: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TileLength",
            tag: 323,
            dtype: &[IfdValueType::Short, IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Tile length",
            long_description: "The TileLength field contains the length of the tiles in the tiled image. TileLength must be a multiple of 16.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 15:  Tiled Images\", p. 66",
        }
    ;

    
        /// Offsets to tile data
        ///
        /// The TileOffsets field contains a value for each tile that is the offset to the beginning of the tile data. The tiles per image is determined from the image dimensions and the tile dimensions.  See ImageWidth, ImageLength, TileWidth, TileLength. The tiles are in order from left-to-right and then top-to-bottom. If the image has PlanarConfiguration==Planar then there is an offset for each sample for each tile of the image, with the offsets pointing to in order the tiles for each separated component.
        ///
        /// references:  \
        /// See also<a href="#TileByteCounts">TileByteCounts</a>, <a href="#TileWidth">TileWidth</a>, <a href="#TileLength">TileLength</a>. <a href="#TIFF6">TIFF6</a>, "Section 15:  Tiled Images", p. 66
        pub const TileOffsets: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TileOffsets",
            tag: 324,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Offsets { lengths: &TileByteCounts },
            count: IfdCount::N,
            description: "Offsets to tile data",
            long_description: "The TileOffsets field contains a value for each tile that is the offset to the beginning of the tile data. The tiles per image is determined from the image dimensions and the tile dimensions.  See ImageWidth, ImageLength, TileWidth, TileLength. The tiles are in order from left-to-right and then top-to-bottom. If the image has PlanarConfiguration==Planar then there is an offset for each sample for each tile of the image, with the offsets pointing to in order the tiles for each separated component.",
            references: "See also<a href=\"#TileByteCounts\">TileByteCounts</a>, <a href=\"#TileWidth\">TileWidth</a>, <a href=\"#TileLength\">TileLength</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 15:  Tiled Images\", p. 66",
        }
    ;

    
        /// Byte counts of tile data
        ///
        /// The TileByteCounts field contains a value for each tile that is the offset to the beginning of the tile data. The tiles per image is determined from the image dimensions and the tile dimensions.  See ImageWidth, ImageLength, TileWidth, TileLength. The tiles are in order from left-to-right and then top-to-bottom. If the image has PlanarConfiguration==Planar then there is an offset for each sample for each tile of the image, with the offsets pointing to in order the tiles for each separated component.
        ///
        /// references:  \
        /// See also<a href="#TileOffsets">TileOffsets</a>, <a href="#TileWidth">TileWidth</a>, <a href="#TileLength">TileLength</a>. <a href="#TIFF6">TIFF6</a>, "Section 15:  Tiled Images", p. 66
        pub const TileByteCounts: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TileByteCounts",
            tag: 325,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Lengths,
            count: IfdCount::N,
            description: "Byte counts of tile data",
            long_description: "The TileByteCounts field contains a value for each tile that is the offset to the beginning of the tile data. The tiles per image is determined from the image dimensions and the tile dimensions.  See ImageWidth, ImageLength, TileWidth, TileLength. The tiles are in order from left-to-right and then top-to-bottom. If the image has PlanarConfiguration==Planar then there is an offset for each sample for each tile of the image, with the offsets pointing to in order the tiles for each separated component.",
            references: "See also<a href=\"#TileOffsets\">TileOffsets</a>, <a href=\"#TileWidth\">TileWidth</a>, <a href=\"#TileLength\">TileLength</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 15:  Tiled Images\", p. 66",
        }
    ;

    
        /// Bad received fax lines
        ///
        /// The BadFaxLines field contains how many of the ImageLength many rows were damaged on facsimile transmission.
        ///
        /// references:  \
        /// <a href="#TIFFSF">TIFFSF</a>
        pub const BadFaxLines: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BadFaxLines",
            tag: 326,
            dtype: &[IfdValueType::Short, IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Bad received fax lines",
            long_description: "The BadFaxLines field contains how many of the ImageLength many rows were damaged on facsimile transmission.",
            references: "<a href=\"#TIFFSF\">TIFFSF</a>",
        }
    ;

    
        /// Fax data cleanliness
        ///
        /// The CleanFaxData field describes the damaged or undamaged state of transmitted Group 3 facsimile data.
        ///
        /// references:  \
        /// <a href="#TIFFSF">TIFFSF</a>
        pub const CleanFaxData: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CleanFaxData",
            tag: 327,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Clean"), (1, "Regenerated"), (2, "Unregenerated"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Fax data cleanliness",
            long_description: "The CleanFaxData field describes the damaged or undamaged state of transmitted Group 3 facsimile data.",
            references: "<a href=\"#TIFFSF\">TIFFSF</a>",
        }
    ;

    
        /// Maximum consecutive bad fax lines
        ///
        /// The ConsecutiveBadFaxLines field contains the maximum of how many facsimile lines in a row were damaged or unreadable on transmission.
        ///
        /// references:  \
        /// <a href="#TIFFSF">TIFFSF</a>
        pub const ConsecutiveBadFaxLines: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ConsecutiveBadFaxLines",
            tag: 328,
            dtype: &[IfdValueType::Long, IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Maximum consecutive bad fax lines",
            long_description: "The ConsecutiveBadFaxLines field contains the maximum of how many facsimile lines in a row were damaged or unreadable on transmission.",
            references: "<a href=\"#TIFFSF\">TIFFSF</a>",
        }
    ;

    
        /// Offsets to child IFDs
        ///
        /// The SubIFDs fields contains offsets to child IFDs of the current IFD that are not otherwise linked in the IFD chain.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 2:  TIFF Structure", p. 14///  <a href="#TTN1">TTN1</a> ///  <a href="#TIFFPM6">TIFFPM6</a>, "TIFF Tech Note 1:  TIFF Trees", p. 4
        pub const SubIFDs: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubIFDs",
            tag: 330,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::IfdOffset { ifd_type: IfdType::Ifd },
            count: IfdCount::N,
            description: "Offsets to child IFDs",
            long_description: "The SubIFDs fields contains offsets to child IFDs of the current IFD that are not otherwise linked in the IFD chain.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 2:  TIFF Structure\", p. 14\n <a href=\"#TTN1\">TTN1</a> \n <a href=\"#TIFFPM6\">TIFFPM6</a>, \"TIFF Tech Note 1:  TIFF Trees\", p. 4",
        }
    ;

    
        /// Ink set
        ///
        /// The InkSet field for separated images describes whether the inkset is CMYK, or not.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 16:  CMYK Images", p. 70
        pub const InkSet: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "InkSet",
            tag: 332,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "CMYK (CMYK inkset)"), (2, "NotCMYK (Not a CMYK inkset)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Ink set",
            long_description: "The InkSet field for separated images describes whether the inkset is CMYK, or not.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 16:  CMYK Images\", p. 70",
        }
    ;

    
        /// Ink names
        ///
        /// The InkNames field contains a null-separated list of strings that define ink/colorant names. The number of strings must be equal to the value of the NumberOfInks field.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 16:  CMYK Images", p. 70
        pub const InkNames: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "InkNames",
            tag: 333,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Ink names",
            long_description: "The InkNames field contains a null-separated list of strings that define ink/colorant names. The number of strings must be equal to the value of the NumberOfInks field.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 16:  CMYK Images\", p. 70",
        }
    ;

    
        /// Ink count
        ///
        /// The NumberOfInks fields contains the number of inks for a separated image.
        ///
        /// references:  \
        /// See also<a href="#InkSet">InkSet</a>, <a href="#InkNames">InkNames</a>. <a href="#TIFF6">TIFF6</a>, "Section 16:  CMYK Images", p. 70
        pub const NumberOfInks: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "NumberOfInks",
            tag: 334,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Ink count",
            long_description: "The NumberOfInks fields contains the number of inks for a separated image.",
            references: "See also<a href=\"#InkSet\">InkSet</a>, <a href=\"#InkNames\">InkNames</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 16:  CMYK Images\", p. 70",
        }
    ;

    
        /// Ink range limits
        ///
        /// The DotRange value contains the value of the sample for 0% and 100% saturation, for either all samples or each sample.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 16:  CMYK Images", p. 71
        pub const DotRange: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DotRange",
            tag: 336,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Ink range limits",
            long_description: "The DotRange value contains the value of the sample for 0% and 100% saturation, for either all samples or each sample.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 16:  CMYK Images\", p. 71",
        }
    ;

    
        /// Target printer
        ///
        /// The TargetPrinter describes the output printer environment.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 16:  CMYK Images", p. 71
        pub const TargetPrinter: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TargetPrinter",
            tag: 337,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Target printer",
            long_description: "The TargetPrinter describes the output printer environment.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 16:  CMYK Images\", p. 71",
        }
    ;

    
        /// Description of extra components
        ///
        /// The ExtraSamples field defines how many data samples there are beyond those for the photometric interpretation. There may be extra components of the image beyond those for the PhotometricInterpretation.  These components are often used to represent transparency information of the samples. For each of the N many extra components, the value of this field represents the meaning of the extra sample.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, p. 31///  <a href="#TIFF6">TIFF6</a>, p. 69///  <a href="#TIFF6">TIFF6</a>, p. 77
        pub const ExtraSamples: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExtraSamples",
            tag: 338,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0x0000, "Unspecified (Unspecified data)"), (0x0001, "AssociatedAlpha (Associated alpha (transparency), premultiplied)"), (0x0002, "UnassociatedAlpha (Unassociated alpha (transparency))"), ] },
            count: IfdCount::N,
            description: "Description of extra components",
            long_description: "The ExtraSamples field defines how many data samples there are beyond those for the photometric interpretation. There may be extra components of the image beyond those for the PhotometricInterpretation.  These components are often used to represent transparency information of the samples. For each of the N many extra components, the value of this field represents the meaning of the extra sample.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, p. 31\n <a href=\"#TIFF6\">TIFF6</a>, p. 69\n <a href=\"#TIFF6\">TIFF6</a>, p. 77",
        }
    ;

    
        /// Format of data sample
        ///
        /// The SampleFormat field represents for each sample the data format of the sample, whether unsigned or signed integer, floating point, or undefined.  The bits per sample are determined by the BitsPerSample field. The default is unsigned integer data, and undefined data should be left or treated as unsigned integer data.
        ///
        /// references:  \
        /// See also<a href="#BitsPerSample">BitsPerSample</a>, <a href="#SMinSampleValue">SMinSampleValue</a>, <a href="#SMaxSampleValue">SMaxSampleValue</a>. <a href="#TIFF6">TIFF6</a>, "Section 19:  Data Sample Format", p. 80
        pub const SampleFormat: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SampleFormat",
            tag: 339,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "UnsignedInteger (Unsigned integer data)"), (2, "SignedInteger (Signed (two's complement) integer data)"), (3, "FloatingPoint (IEEE floating point data)"), (4, "Undefined (Undefined sample format)"), ] },
            count: IfdCount::N,
            description: "Format of data sample",
            long_description: "The SampleFormat field represents for each sample the data format of the sample, whether unsigned or signed integer, floating point, or undefined.  The bits per sample are determined by the BitsPerSample field. The default is unsigned integer data, and undefined data should be left or treated as unsigned integer data.",
            references: "See also<a href=\"#BitsPerSample\">BitsPerSample</a>, <a href=\"#SMinSampleValue\">SMinSampleValue</a>, <a href=\"#SMaxSampleValue\">SMaxSampleValue</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 19:  Data Sample Format\", p. 80",
        }
    ;

    
        /// Minimum sample value of data format
        ///
        /// The SMinSampleValue field identifies the least sample value for each sample, from the range of values possible given the bits per sample of the sample as its type as specified by the SampleFormat field.
        ///
        /// references:  \
        /// See also<a href="#SMaxSampleValue">SMaxSampleValue</a>. <a href="#TIFF6">TIFF6</a>, "Section 19:  Data Sample Format", p. 80
        pub const SMinSampleValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SMinSampleValue",
            tag: 340,
            dtype: &[IfdValueType::Long, IfdValueType::Short, IfdValueType::Byte, IfdValueType::SLong, IfdValueType::SShort, IfdValueType::SByte, IfdValueType::Double, IfdValueType::Float, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Minimum sample value of data format",
            long_description: "The SMinSampleValue field identifies the least sample value for each sample, from the range of values possible given the bits per sample of the sample as its type as specified by the SampleFormat field.",
            references: "See also<a href=\"#SMaxSampleValue\">SMaxSampleValue</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 19:  Data Sample Format\", p. 80",
        }
    ;

    
        /// Maximum sample value of data format
        ///
        /// The SMaxSampleValue field identifies the greatest sample value for each sample, from the range of values possible given the bits per sample of the sample as its type as specified by the SampleFormat field.
        ///
        /// references:  \
        /// See also<a href="#SMinSampleValue">SMinSampleValue</a>. <a href="#TIFF6">TIFF6</a>, "Section 19:  Data Sample Format", p. 80
        pub const SMaxSampleValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SMaxSampleValue",
            tag: 341,
            dtype: &[IfdValueType::Long, IfdValueType::Short, IfdValueType::Byte, IfdValueType::SLong, IfdValueType::SShort, IfdValueType::SByte, IfdValueType::Double, IfdValueType::Float, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Maximum sample value of data format",
            long_description: "The SMaxSampleValue field identifies the greatest sample value for each sample, from the range of values possible given the bits per sample of the sample as its type as specified by the SampleFormat field.",
            references: "See also<a href=\"#SMinSampleValue\">SMinSampleValue</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 19:  Data Sample Format\", p. 80",
        }
    ;

    
        /// Transfer range
        ///
        /// The TransferRange field contains two values for each of three components that are an offset and a scaling, these values expand the TransferFunction range.
        ///
        /// references:  \
        /// See also<a href="#TransferFunction">TransferFunction</a>. <a href="#TIFF6">TIFF6</a>, "Section 20:  RGB Image Colorimetry", p. 85
        pub const TransferRange: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TransferRange",
            tag: 342,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(6),
            description: "Transfer range",
            long_description: "The TransferRange field contains two values for each of three components that are an offset and a scaling, these values expand the TransferFunction range.",
            references: "See also<a href=\"#TransferFunction\">TransferFunction</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 20:  RGB Image Colorimetry\", p. 85",
        }
    ;

    
        /// Clipping path
        ///
        /// The ClipPath field contains the specification of a clipping path that outlines the image data to be output. The TIFF clipping path is designed to be directly compatible with PostScript path information. The clip path contains a header, 16 bytes with "II" or "MM" and then zeros, and then path operators, commands.
        ///
        /// references:  \
        /// <a href="#TIFFPM6">TIFFPM6</a>, "TIFF Tech Note 2:  Clipping Path", p. 6///  <a href="#PLRM">PLRM</a>
        pub const ClipPath: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ClipPath",
            tag: 343,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Clipping path",
            long_description: "The ClipPath field contains the specification of a clipping path that outlines the image data to be output. The TIFF clipping path is designed to be directly compatible with PostScript path information. The clip path contains a header, 16 bytes with \"II\" or \"MM\" and then zeros, and then path operators, commands.",
            references: "<a href=\"#TIFFPM6\">TIFFPM6</a>, \"TIFF Tech Note 2:  Clipping Path\", p. 6\n <a href=\"#PLRM\">PLRM</a>",
        }
    ;

    
        /// Clipping path horizontal units
        ///
        /// The XClipPathUnits field contains the number of horizontal clip path coordinates, the count of coordinates for the clip path coordinate system across the image.
        ///
        /// references:  \
        /// <a href="#TIFFPM6">TIFFPM6</a>, "TIFF Tech Note 2:  Clipping Path", p. 6
        pub const XClipPathUnits: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "XClipPathUnits",
            tag: 344,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Clipping path horizontal units",
            long_description: "The XClipPathUnits field contains the number of horizontal clip path coordinates, the count of coordinates for the clip path coordinate system across the image.",
            references: "<a href=\"#TIFFPM6\">TIFFPM6</a>, \"TIFF Tech Note 2:  Clipping Path\", p. 6",
        }
    ;

    
        /// Clipping path vertical units
        ///
        /// The YClipPathUnits field contains the number of vertical clip path coordinates, the count of coordinates for the clip path coordinate system down the image.
        ///
        /// references:  \
        /// <a href="#TIFFPM6">TIFFPM6</a>, "TIFF Tech Note 2:  Clipping Path", p. 6
        pub const YClipPathUnits: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "YClipPathUnits",
            tag: 345,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Clipping path vertical units",
            long_description: "The YClipPathUnits field contains the number of vertical clip path coordinates, the count of coordinates for the clip path coordinate system down the image.",
            references: "<a href=\"#TIFFPM6\">TIFFPM6</a>, \"TIFF Tech Note 2:  Clipping Path\", p. 6",
        }
    ;

    
        /// Indexed (palettized) image
        ///
        /// The Indexed field denotes whether the image data given the photometric interpretation (see PhotometricInterpretation) is palettized or indexed to the color map (see ColorMap).
        ///
        /// references:  \
        /// See also<a href="#PhotometricInterpretation">PhotometricInterpretation</a>, <a href="#ColorMap">ColorMap</a>. <a href="#TIFFPM6">TIFFPM6</a>, "TIFF Tech Note 3:  Indexed Images", p. 11///  <a href="#TIFFFX">TIFFFX</a>
        pub const Indexed: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Indexed",
            tag: 346,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0x0000, "NotIndexed (Not indexed)"), (0x0001, "Indexed (Indexed)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Indexed (palettized) image",
            long_description: "The Indexed field denotes whether the image data given the photometric interpretation (see PhotometricInterpretation) is palettized or indexed to the color map (see ColorMap).",
            references: "See also<a href=\"#PhotometricInterpretation\">PhotometricInterpretation</a>, <a href=\"#ColorMap\">ColorMap</a>. <a href=\"#TIFFPM6\">TIFFPM6</a>, \"TIFF Tech Note 3:  Indexed Images\", p. 11\n <a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// Contents of new JPEG tables
        ///
        /// The JPEGTables field contains an abbreviated JPEG data table specification to install the tables into the JPEG coder for JPEG coded data.
        ///
        /// references:  \
        /// <a href="#TTN2">TTN2</a>
        pub const JPEGTables: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "JPEGTables",
            tag: 347,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Contents of new JPEG tables",
            long_description: "The JPEGTables field contains an abbreviated JPEG data table specification to install the tables into the JPEG coder for JPEG coded data.",
            references: "<a href=\"#TTN2\">TTN2</a>",
        }
    ;

    
        /// OPI proxy indicator
        ///
        /// The OPIProxy field denotes whether this image is a proxy for a higher resolution image named in the ImageID field.
        ///
        /// references:  \
        /// See also<a href="#ImageID">ImageID</a>. <a href="#TIFFPM6">TIFFPM6</a>, p. 15///  <a href="#OPI2">OPI2</a>
        pub const OPIProxy: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "OPIProxy",
            tag: 351,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "OPI proxy indicator",
            long_description: "The OPIProxy field denotes whether this image is a proxy for a higher resolution image named in the ImageID field.",
            references: "See also<a href=\"#ImageID\">ImageID</a>. <a href=\"#TIFFPM6\">TIFFPM6</a>, p. 15\n <a href=\"#OPI2\">OPI2</a>",
        }
    ;

    
        /// TIFF-FX decode array
        ///
        /// The Decode field contains values that define the range of colorspace values to map the range of sample values.
        ///
        /// references:  \
        /// <a href="#TIFFFX">TIFFFX</a>
        pub const Decode: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Decode",
            tag: 385,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF-FX decode array",
            long_description: "The Decode field contains values that define the range of colorspace values to map the range of sample values.",
            references: "<a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// TIFF-FX default image color
        ///
        /// The DefaultImageColor field contains the samples of a pixel that define the default image color in TIFF-FX MRC image data.
        ///
        /// references:  \
        /// <a href="#TIFFFX">TIFFFX</a>
        pub const DefaultImageColor: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DefaultImageColor",
            tag: 386,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF-FX default image color",
            long_description: "The DefaultImageColor field contains the samples of a pixel that define the default image color in TIFF-FX MRC image data.",
            references: "<a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// TIFF-FX IFD offset for global parameters
        ///
        /// The GlobalParametersIFD field is to contain an offset to a child IFD that contains fields with global parameters of the TIFF-FX profile file. The GlobalParametersIFD field should be written to the first IFD of the TIFF-FX file.  The global parameters IFD should contain ProfileType, FaxProfile, CodingMethods, VersionYear, and ModeNumber fields.
        ///
        /// references:  \
        /// See also<a href="#ProfileType">ProfileType</a>, <a href="#FaxProfile">FaxProfile</a>, <a href="#CodingMethods">CodingMethods</a>, <a href="#VersionYear">VersionYear</a>, <a href="#ModeNumber">ModeNumber</a>. <a href="#TIFFFX">TIFFFX</a>
        pub const GlobalParametersIFD: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GlobalParametersIFD",
            tag: 400,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::IfdOffset { ifd_type: IfdType::Ifd },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF-FX IFD offset for global parameters",
            long_description: "The GlobalParametersIFD field is to contain an offset to a child IFD that contains fields with global parameters of the TIFF-FX profile file. The GlobalParametersIFD field should be written to the first IFD of the TIFF-FX file.  The global parameters IFD should contain ProfileType, FaxProfile, CodingMethods, VersionYear, and ModeNumber fields.",
            references: "See also<a href=\"#ProfileType\">ProfileType</a>, <a href=\"#FaxProfile\">FaxProfile</a>, <a href=\"#CodingMethods\">CodingMethods</a>, <a href=\"#VersionYear\">VersionYear</a>, <a href=\"#ModeNumber\">ModeNumber</a>. <a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// TIFF-FX profile type
        ///
        /// The ProfileType field describes the profile of data in the TIFF file.
        ///
        /// references:  \
        /// See also<a href="#GlobalParametersIFD">GlobalParametersIFD</a>. <a href="#TIFFFX">TIFFFX</a>
        pub const ProfileType: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ProfileType",
            tag: 401,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Unspecified"), (1, "Group3Fax"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF-FX profile type",
            long_description: "The ProfileType field describes the profile of data in the TIFF file.",
            references: "See also<a href=\"#GlobalParametersIFD\">GlobalParametersIFD</a>. <a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// TIFF-FX fax profile
        ///
        /// The FaxProfile field describes the TIFF-FX profile of data in the TIFF file.
        ///
        /// references:  \
        /// See also<a href="#GlobalParametersIFD">GlobalParametersIFD</a>, <a href="#MultiProfiles">MultiProfiles</a>. <a href="#TIFFFX">TIFFFX</a> ///  <a href="#TIFFFXEX1">TIFFFXEX1</a>
        pub const FaxProfile: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FaxProfile",
            tag: 402,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "NotProfile"), (1, "ProfileS"), (2, "ProfileF"), (3, "ProfileJ"), (4, "ProfileC"), (5, "ProfileL"), (6, "ProfileM"), (7, "ProfileT"), (255, "MultiProfiles"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF-FX fax profile",
            long_description: "The FaxProfile field describes the TIFF-FX profile of data in the TIFF file.",
            references: "See also<a href=\"#GlobalParametersIFD\">GlobalParametersIFD</a>, <a href=\"#MultiProfiles\">MultiProfiles</a>. <a href=\"#TIFFFX\">TIFFFX</a> \n <a href=\"#TIFFFXEX1\">TIFFFXEX1</a>",
        }
    ;

    
        /// TIFF-FX coding methods
        ///
        /// The CodingMethods field describes the coding methods used on the data in the TIFF-FX file.
        ///
        /// references:  \
        /// See also<a href="#GlobalParametersIFD">GlobalParametersIFD</a>. <a href="#TIFFFX">TIFFFX</a>
        pub const CodingMethods: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CodingMethods",
            tag: 403,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Bitflags { values: &[(0, "Unspecified"), (1, "Group3_1D"), (2, "Group3_2D"), (3, "Group4"), (4, "JBIG"), (5, "JPEG"), (6, "JBIGColor"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF-FX coding methods",
            long_description: "The CodingMethods field describes the coding methods used on the data in the TIFF-FX file.",
            references: "See also<a href=\"#GlobalParametersIFD\">GlobalParametersIFD</a>. <a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// TIFF-FX version year
        ///
        /// The VersionYear field contains four characters forming the ASCII representation of the TIFF-FX version.
        ///
        /// references:  \
        /// See also<a href="#GlobalParametersIFD">GlobalParametersIFD</a>. <a href="#TIFFFX">TIFFFX</a>
        pub const VersionYear: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "VersionYear",
            tag: 404,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(4),
            description: "TIFF-FX version year",
            long_description: "The VersionYear field contains four characters forming the ASCII representation of the TIFF-FX version.",
            references: "See also<a href=\"#GlobalParametersIFD\">GlobalParametersIFD</a>. <a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// TIFF-FX mode number
        ///
        /// The ModeNumber field describes the mode of the standard used by the TIFF-FX FaxProfile field.
        ///
        /// references:  \
        /// See also<a href="#GlobalParametersIFD">GlobalParametersIFD</a>, <a href="#FaxProfile">FaxProfile</a>. <a href="#TIFFFX">TIFFFX</a>
        pub const ModeNumber: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ModeNumber",
            tag: 405,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Version_1_0 (FaxProfile Mode 1.0)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF-FX mode number",
            long_description: "The ModeNumber field describes the mode of the standard used by the TIFF-FX FaxProfile field.",
            references: "See also<a href=\"#GlobalParametersIFD\">GlobalParametersIFD</a>, <a href=\"#FaxProfile\">FaxProfile</a>. <a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// TIFF-FX T.82 options
        ///
        /// The T82Options field contains options of the ITU-T T.82 (JBIG) coding.
        ///
        /// references:  \
        /// <a href="#TIFFFXEX1">TIFFFXEX1</a>
        pub const T82Options: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "T82Options",
            tag: 435,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Bitflags { values: &[] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF-FX T.82 options",
            long_description: "The T82Options field contains options of the ITU-T T.82 (JBIG) coding.",
            references: "<a href=\"#TIFFFXEX1\">TIFFFXEX1</a>",
        }
    ;

    
        /// JPEG Process
        ///
        /// The JPEGProc field defines the JPEG process used to compress the image data, as either baseline sequential or lossless.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 22:  JPEG Compression", p. 104
        pub const JPEGProc: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "JPEGProc",
            tag: 512,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "BaselineSequential (Baseline sequential JPEG process)"), (14, "LosslessHuffman (Lossless JPEG process with Huffman encoding)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "JPEG Process",
            long_description: "The JPEGProc field defines the JPEG process used to compress the image data, as either baseline sequential or lossless.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 22:  JPEG Compression\", p. 104",
        }
    ;

    
        /// Offset to JPEG interchange format
        ///
        /// The JPEGInterchangeFormat field contains an offset to the beginning of a block of JPEG interchange format data.
        ///
        /// references:  \
        /// See also<a href="#JPEGInterchangeFormatLength">JPEGInterchangeFormatLength</a>. <a href="#TIFF6">TIFF6</a>, "Section 22:  JPEG Compression", p. 105
        pub const JPEGInterchangeFormat: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "JPEGInterchangeFormat",
            tag: 513,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Offsets { lengths: &JPEGInterchangeFormatLength },
            count: IfdCount::ConcreteValue(1),
            description: "Offset to JPEG interchange format",
            long_description: "The JPEGInterchangeFormat field contains an offset to the beginning of a block of JPEG interchange format data.",
            references: "See also<a href=\"#JPEGInterchangeFormatLength\">JPEGInterchangeFormatLength</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 22:  JPEG Compression\", p. 105",
        }
    ;

    
        /// Byte count of JPEG interchange format
        ///
        /// The JPEGInterchangeFormatLength describes the extent of a block of JPEG interchange format data from the offset of the data.
        ///
        /// references:  \
        /// See also<a href="#JPEGInterchangeFormat">JPEGInterchangeFormat</a>. <a href="#TIFF6">TIFF6</a>, "Section 22:  JPEG Compression", p. 105
        pub const JPEGInterchangeFormatLength: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "JPEGInterchangeFormatLength",
            tag: 514,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Lengths,
            count: IfdCount::ConcreteValue(1),
            description: "Byte count of JPEG interchange format",
            long_description: "The JPEGInterchangeFormatLength describes the extent of a block of JPEG interchange format data from the offset of the data.",
            references: "See also<a href=\"#JPEGInterchangeFormat\">JPEGInterchangeFormat</a>. <a href=\"#TIFF6\">TIFF6</a>, \"Section 22:  JPEG Compression\", p. 105",
        }
    ;

    
        /// JPEG restart interval
        ///
        /// The JPEGRestartInterval field contains the value of the restart interval of the JPEG coded data.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 22:  JPEG Compression", p. 105
        pub const JPEGRestartInterval: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "JPEGRestartInterval",
            tag: 515,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "JPEG restart interval",
            long_description: "The JPEGRestartInterval field contains the value of the restart interval of the JPEG coded data.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 22:  JPEG Compression\", p. 105",
        }
    ;

    
        /// JPEG lossless predictors
        ///
        /// The JPEGLosslessPredictors field contains a list of the lossless predictor selection values of the JPEG coded data.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 22:  JPEG Compression", p. 106
        pub const JPEGLosslessPredictors: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "JPEGLosslessPredictors",
            tag: 517,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "JPEG lossless predictors",
            long_description: "The JPEGLosslessPredictors field contains a list of the lossless predictor selection values of the JPEG coded data.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 22:  JPEG Compression\", p. 106",
        }
    ;

    
        /// JPEG point transforms
        ///
        /// The JPEGPointTransforms field contains a list of the point transforms of the JPEG coded data.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 22:  JPEG Compression", p. 106
        pub const JPEGPointTransforms: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "JPEGPointTransforms",
            tag: 518,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "JPEG point transforms",
            long_description: "The JPEGPointTransforms field contains a list of the point transforms of the JPEG coded data.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 22:  JPEG Compression\", p. 106",
        }
    ;

    
        /// Offsets to JPEG quantization tables
        ///
        /// The JPEGQTables field contains a list of offsets to quantization tables of the JPEG coded data. Each table is 64 bytes in extent.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 22:  JPEG Compression", p. 107
        pub const JPEGQTables: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "JPEGQTables",
            tag: 519,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Offsets to JPEG quantization tables",
            long_description: "The JPEGQTables field contains a list of offsets to quantization tables of the JPEG coded data. Each table is 64 bytes in extent.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 22:  JPEG Compression\", p. 107",
        }
    ;

    
        /// Offsets to JPEG DC tables
        ///
        /// The JPEGDCTables field contains a list of offsets to Huffman DC tables of the JPEG coded data. The extent of each is 16 bytes plus the sum of the values in those 16 bytes.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 22:  JPEG Compression", p. 107
        pub const JPEGDCTables: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "JPEGDCTables",
            tag: 520,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Offsets to JPEG DC tables",
            long_description: "The JPEGDCTables field contains a list of offsets to Huffman DC tables of the JPEG coded data. The extent of each is 16 bytes plus the sum of the values in those 16 bytes.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 22:  JPEG Compression\", p. 107",
        }
    ;

    
        /// Offsets to JPEG AC tables
        ///
        /// The JPEGACTables field contains a list of offsets to Huffman AC tables of the JPEG coded data. The extent of each is 16 bytes plus the sum of the values in those 16 bytes.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 22:  JPEG Compression", p. 107
        pub const JPEGACTables: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "JPEGACTables",
            tag: 521,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Offsets to JPEG AC tables",
            long_description: "The JPEGACTables field contains a list of offsets to Huffman AC tables of the JPEG coded data. The extent of each is 16 bytes plus the sum of the values in those 16 bytes.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 22:  JPEG Compression\", p. 107",
        }
    ;

    
        /// Transformation from RGB to YCbCr
        ///
        /// The YCbCrCoefficients fields specifies three fractions that represent the coefficients used to generate the luminance channel, Y, of YCbCr data, from RGB data.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 21:  YCbCr Images", p. 90
        pub const YCbCrCoefficients: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "YCbCrCoefficients",
            tag: 529,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(3),
            description: "Transformation from RGB to YCbCr",
            long_description: "The YCbCrCoefficients fields specifies three fractions that represent the coefficients used to generate the luminance channel, Y, of YCbCr data, from RGB data.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 21:  YCbCr Images\", p. 90",
        }
    ;

    
        /// Chrominance component subsampling
        ///
        /// The YCbCrSubSampling field contains a value for both of two chromaticity components that describes the horizontal subsampling frequency and the vertical subsampling frequency, horizontal in the first value and vertical in the second value.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 21:  YCbCr Images", p. 91
        pub const YCbCrSubSampling: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "YCbCrSubSampling",
            tag: 530,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "Chrominance component subsampling",
            long_description: "The YCbCrSubSampling field contains a value for both of two chromaticity components that describes the horizontal subsampling frequency and the vertical subsampling frequency, horizontal in the first value and vertical in the second value.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 21:  YCbCr Images\", p. 91",
        }
    ;

    
        /// Position of chrominance to luminance samples
        ///
        /// The YCbCrPositioning field describes whether the chrominance channels are centered among the luminance channels or cosited upon subsampling of the chrominance channels.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 21:  YCbCr Images", p. 92
        pub const YCbCrPositioning: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "YCbCrPositioning",
            tag: 531,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "Centered"), (2, "Cosited"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Position of chrominance to luminance samples",
            long_description: "The YCbCrPositioning field describes whether the chrominance channels are centered among the luminance channels or cosited upon subsampling of the chrominance channels.",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 21:  YCbCr Images\", p. 92",
        }
    ;

    
        /// Reference black and white
        ///
        /// The ReferenceBlackWhite field contains two values for each of three primaries that specify headroom and footroom values for each of three primaries (channels).
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>, "Section 20:  RGB Image Colorimetry", p. 86
        pub const ReferenceBlackWhite: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ReferenceBlackWhite",
            tag: 532,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(6),
            description: "Reference black and white",
            long_description: "The ReferenceBlackWhite field contains two values for each of three primaries that specify headroom and footroom values for each of three primaries (channels).",
            references: "<a href=\"#TIFF6\">TIFF6</a>, \"Section 20:  RGB Image Colorimetry\", p. 86",
        }
    ;

    
        /// TIFF-FX rows per strips
        ///
        /// The StripRowCounts field contains a count for each strip of the number of rows in that strip for use with TIFF-FX MRC data, which can have a variable number of row per strip. If the StripRowCounts field is present then the RowsPerStrip field is to not be.
        ///
        /// references:  \
        /// <a href="#TIFFFX">TIFFFX</a>
        pub const StripRowCounts: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "StripRowCounts",
            tag: 559,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF-FX rows per strips",
            long_description: "The StripRowCounts field contains a count for each strip of the number of rows in that strip for use with TIFF-FX MRC data, which can have a variable number of row per strip. If the StripRowCounts field is present then the RowsPerStrip field is to not be.",
            references: "<a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// XMP (XML) packet
        ///
        /// The XMLPacket field contains embedded XMP (XML/RDF) metadata about the information.
        ///
        /// references:  \
        /// <a href="#XMPEMBED">XMPEMBED</a>
        pub const XMLPacket: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "XMLPacket",
            tag: 700,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Blob,
            count: IfdCount::N,
            description: "XMP (XML) packet",
            long_description: "The XMLPacket field contains embedded XMP (XML/RDF) metadata about the information.",
            references: "<a href=\"#XMPEMBED\">XMPEMBED</a>",
        }
    ;

    
        /// USPTO Miscellaneous (private tag)
        ///
        /// The USPTO Miscellaenous field is by default blank.
        ///
        /// references:  \
        /// <a href="#YB2">YB2</a>, p. 7
        pub const USPTOMiscellaneous: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "USPTOMiscellaneous",
            tag: 999,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(253),
            description: "USPTO Miscellaneous (private tag)",
            long_description: "The USPTO Miscellaenous field is by default blank.",
            references: "<a href=\"#YB2\">YB2</a>, p. 7",
        }
    ;

    
        /// OPI image identifier
        ///
        /// The ImageID tag contains a filename or other identifier of the high resolution original of this image.
        ///
        /// references:  \
        /// See also<a href="#OPIProxy">OPIProxy</a>. <a href="#TIFFPM6">TIFFPM6</a>, p. 15///  <a href="#OPI2">OPI2</a>
        pub const ImageID: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageID",
            tag: 32781,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "OPI image identifier",
            long_description: "The ImageID tag contains a filename or other identifier of the high resolution original of this image.",
            references: "See also<a href=\"#OPIProxy\">OPIProxy</a>. <a href=\"#TIFFPM6\">TIFFPM6</a>, p. 15\n <a href=\"#OPI2\">OPI2</a>",
        }
    ;

    
        /// Wang Imaging (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const WangTag1: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "WangTag1",
            tag: 32931,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Wang Imaging (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// Wang Imaging annotation
        ///
        /// The WangAnnotation field contains the content of data structures that define annotations to the image per the Wang/Eastman/Kodak/eiStream annotation specification.
        ///
        /// references:  \
        /// <a href="#WANGANNO">WANGANNO</a>
        pub const WangAnnotation: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "WangAnnotation",
            tag: 32932,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Wang Imaging annotation",
            long_description: "The WangAnnotation field contains the content of data structures that define annotations to the image per the Wang/Eastman/Kodak/eiStream annotation specification.",
            references: "<a href=\"#WANGANNO\">WANGANNO</a>",
        }
    ;

    
        /// Wang Imaging (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const WangTag3: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "WangTag3",
            tag: 32933,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Wang Imaging (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// Wang Imaging (private tag)
        ///
        /// The WangTag field contains some information put into TIFF files by Wang Imaging software.
        ///
        /// references:  \
        /// 
        pub const WangTag4: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "WangTag4",
            tag: 32934,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Wang Imaging (private tag)",
            long_description: "The WangTag field contains some information put into TIFF files by Wang Imaging software.",
            references: "",
        }
    ;

    
        /// TIFF/EP color filter array dimensions
        ///
        /// The CFARepeatPatternDim field contains two values representing the minimum rows and columns to define the repeating patterns of the color filter array.
        ///
        /// references:  \
        /// See also<a href="#CFAPattern">CFAPattern</a>, <a href="#SensingMethod">SensingMethod</a>. <a href="#TIFFEP">TIFFEP</a>
        pub const CFARepeatPatternDim: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CFARepeatPatternDim",
            tag: 33421,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "TIFF/EP color filter array dimensions",
            long_description: "The CFARepeatPatternDim field contains two values representing the minimum rows and columns to define the repeating patterns of the color filter array.",
            references: "See also<a href=\"#CFAPattern\">CFAPattern</a>, <a href=\"#SensingMethod\">SensingMethod</a>. <a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture color filter array pattern
        ///
        /// The CFAPattern field contains a description of the color filter array geometric pattern for interleaving of sampling channels.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const CFAPattern: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CFAPattern",
            tag: 33422,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::CfaPattern,
            count: IfdCount::N,
            description: "TIFF/EP picture color filter array pattern",
            long_description: "The CFAPattern field contains a description of the color filter array geometric pattern for interleaving of sampling channels.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP battery level
        ///
        /// The BatteryLevel field contains a value of the battery level as a fraction or string.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const BatteryLevel: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BatteryLevel",
            tag: 33423,
            dtype: &[IfdValueType::Rational, IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP battery level",
            long_description: "The BatteryLevel field contains a value of the battery level as a fraction or string.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// Copyright notice
        ///
        /// The Copyright field contains copyright information of the interpreted image data. GEDI standard, gedistand99.pdf, page 39, has value being 0x0828.
        ///
        /// references:  \
        /// <a href="#TIFF6">TIFF6</a>
        pub const Copyright: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Copyright",
            tag: 33432,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Copyright notice",
            long_description: "The Copyright field contains copyright information of the interpreted image data. GEDI standard, gedistand99.pdf, page 39, has value being 0x0828.",
            references: "<a href=\"#TIFF6\">TIFF6</a>",
        }
    ;

    
        /// TIFF/EP picture exposure time
        ///
        /// The ExposureTime field contains how many seconds the frame was exposed.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const ExposureTime: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExposureTime",
            tag: 33434,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture exposure time",
            long_description: "The ExposureTime field contains how many seconds the frame was exposed.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture F number
        ///
        /// The FNumber field contains the F number of the picture.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const FNumber: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FNumber",
            tag: 33437,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture F number",
            long_description: "The FNumber field contains the F number of the picture.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// GeoTIFF model pixel scale
        ///
        /// 
        ///
        /// references:  \
        /// <a href="#GEOTIFF">GEOTIFF</a>
        pub const ModelPixelScaleTag: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ModelPixelScaleTag",
            tag: 33550,
            dtype: &[IfdValueType::Double, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(3),
            description: "GeoTIFF model pixel scale",
            long_description: "",
            references: "<a href=\"#GEOTIFF\">GEOTIFF</a>",
        }
    ;

    
        /// Advent Imaging scale (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const AdventScale: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "AdventScale",
            tag: 33589,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Advent Imaging scale (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// Advent Imaging revision (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const AdventRevision: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "AdventRevision",
            tag: 33590,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Advent Imaging revision (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// IPTC/NAA metadata record
        ///
        /// The IPTCNAA field contains an IPTC/NAA record.
        ///
        /// references:  \
        /// <a href="#RICHTIFF">RICHTIFF</a> ///  <a href="#TIFFEP">TIFFEP</a>
        pub const IPTCNAA: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "IPTCNAA",
            tag: 33723,
            dtype: &[IfdValueType::Byte, IfdValueType::Ascii, IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "IPTC/NAA metadata record",
            long_description: "The IPTCNAA field contains an IPTC/NAA record.",
            references: "<a href=\"#RICHTIFF\">RICHTIFF</a> \n <a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// Intergraph INGR packet data (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const INGRPacketData: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "INGRPacketData",
            tag: 33918,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Intergraph INGR packet data (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// Intergraph INGR flag registers (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const INGRFlagRegisters: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "INGRFlagRegisters",
            tag: 33919,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Intergraph INGR flag registers (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// Intergraph matrix (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const IntergraphMatrix: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "IntergraphMatrix",
            tag: 33920,
            dtype: &[IfdValueType::Double, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Intergraph matrix (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// Intergraph reserved (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const INGRReserved: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "INGRReserved",
            tag: 33921,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Intergraph reserved (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// GeoTIFF model tiepoints
        ///
        /// Also called Georeferencing.
        ///
        /// references:  \
        /// <a href="#GEOTIFF">GEOTIFF</a>
        pub const ModelTiepointTag: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ModelTiepointTag",
            tag: 33922,
            dtype: &[IfdValueType::Double, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "GeoTIFF model tiepoints",
            long_description: "Also called Georeferencing.",
            references: "<a href=\"#GEOTIFF\">GEOTIFF</a>",
        }
    ;

    
        /// TIFF/IT production site
        ///
        /// The Site field contains the location of where the image was originated or converted to TIFF/IT.
        ///
        /// references:  \
        /// <a href="#TIFFIT">TIFFIT</a>
        pub const Site: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Site",
            tag: 34016,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/IT production site",
            long_description: "The Site field contains the location of where the image was originated or converted to TIFF/IT.",
            references: "<a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT color sequence
        ///
        /// The ColorSequence field is a string where each letter signifies a color to be assigned to a component.
        ///
        /// references:  \
        /// <a href="#TIFFIT">TIFFIT</a>
        pub const ColorSequence: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ColorSequence",
            tag: 34017,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/IT color sequence",
            long_description: "The ColorSequence field is a string where each letter signifies a color to be assigned to a component.",
            references: "<a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT header
        ///
        /// The IT8Header field contains null-separated headers from ISO 10755, ISO 10756, and ISO 10759.
        ///
        /// references:  \
        /// <a href="#TIFFIT">TIFFIT</a>
        pub const IT8Header: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "IT8Header",
            tag: 34018,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/IT header",
            long_description: "The IT8Header field contains null-separated headers from ISO 10755, ISO 10756, and ISO 10759.",
            references: "<a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT raster padding
        ///
        /// The RasterPadding field describes the padding of data to byte, word, long word, sector, or double sector. When applied to line interleaved data, the padding applies to each color instead of each line.
        ///
        /// references:  \
        /// See also<a href="#PhotometricInterpretation">PhotometricInterpretation</a>. <a href="#TIFFIT">TIFFIT</a>
        pub const RasterPadding: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "RasterPadding",
            tag: 34019,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "ByteRaster"), (1, "WordRaster"), (2, "LongWordRaster"), (9, "SectorRaster"), (10, "LongSectorRaster"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/IT raster padding",
            long_description: "The RasterPadding field describes the padding of data to byte, word, long word, sector, or double sector. When applied to line interleaved data, the padding applies to each color instead of each line.",
            references: "See also<a href=\"#PhotometricInterpretation\">PhotometricInterpretation</a>. <a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT LW bits per run length
        ///
        /// The BitsPerRunLength field contains how many bits are required to represent the short run of the run length encoding of the TIFF/IT LW (line work) data.
        ///
        /// references:  \
        /// See also<a href="#BitsPerExtendedRunLength">BitsPerExtendedRunLength</a>. <a href="#TIFFIT">TIFFIT</a>
        pub const BitsPerRunLength: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BitsPerRunLength",
            tag: 34020,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/IT LW bits per run length",
            long_description: "The BitsPerRunLength field contains how many bits are required to represent the short run of the run length encoding of the TIFF/IT LW (line work) data.",
            references: "See also<a href=\"#BitsPerExtendedRunLength\">BitsPerExtendedRunLength</a>. <a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT LW bits per extended run length
        ///
        /// The BitsPerRunLength field contains how many bits are required to represent the long run of the run length encoding of the TIFF/IT LW (line work) data.
        ///
        /// references:  \
        /// See also<a href="#BitsPerRunLength">BitsPerRunLength</a>. <a href="#TIFFIT">TIFFIT</a>
        pub const BitsPerExtendedRunLength: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BitsPerExtendedRunLength",
            tag: 34021,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/IT LW bits per extended run length",
            long_description: "The BitsPerRunLength field contains how many bits are required to represent the long run of the run length encoding of the TIFF/IT LW (line work) data.",
            references: "See also<a href=\"#BitsPerRunLength\">BitsPerRunLength</a>. <a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT LW color table
        ///
        /// The ColorTable field contains a color identifier and transparency information for separated line work images.
        ///
        /// references:  \
        /// <a href="#TIFFIT">TIFFIT</a>, p. 17
        pub const ColorTable: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ColorTable",
            tag: 34022,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/IT LW color table",
            long_description: "The ColorTable field contains a color identifier and transparency information for separated line work images.",
            references: "<a href=\"#TIFFIT\">TIFFIT</a>, p. 17",
        }
    ;

    
        /// TIFF/IT BP/BL foreground color indicator
        ///
        /// The ImageColorIndicator field describes whether the image or foreground color is in the binary image or the monochrome continuous tone image.
        ///
        /// references:  \
        /// See also<a href="#BackgroundColorIndicator">BackgroundColorIndicator</a>. <a href="#TIFFIT">TIFFIT</a>
        pub const ImageColorIndicator: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageColorIndicator",
            tag: 34023,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "UnspecifiedImageColor (Image color not specified)"), (1, "SpecifiedImageColor (Image color specified)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/IT BP/BL foreground color indicator",
            long_description: "The ImageColorIndicator field describes whether the image or foreground color is in the binary image or the monochrome continuous tone image.",
            references: "See also<a href=\"#BackgroundColorIndicator\">BackgroundColorIndicator</a>. <a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT BP/BL background color indicator
        ///
        /// The BackgroundColorIndicator field describes whether the image or background color is in the binary image or the monochrome continuous tone image.
        ///
        /// references:  \
        /// See also<a href="#ImageColorIndicator">ImageColorIndicator</a>. <a href="#TIFFIT">TIFFIT</a>
        pub const BackgroundColorIndicator: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BackgroundColorIndicator",
            tag: 34024,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "UnspecifiedBackgroundColor (Background color not specified)"), (1, "SpecifiedBackgroundColor (Background color specified)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/IT BP/BL background color indicator",
            long_description: "The BackgroundColorIndicator field describes whether the image or background color is in the binary image or the monochrome continuous tone image.",
            references: "See also<a href=\"#ImageColorIndicator\">ImageColorIndicator</a>. <a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT BP/BL foreground color value
        ///
        /// The ImageColorValue describes the foreground color of a TIFF/IT BP or BL bitmap.
        ///
        /// references:  \
        /// <a href="#TIFFIT">TIFFIT</a>
        pub const ImageColorValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageColorValue",
            tag: 34025,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/IT BP/BL foreground color value",
            long_description: "The ImageColorValue describes the foreground color of a TIFF/IT BP or BL bitmap.",
            references: "<a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT BP/BL background color value
        ///
        /// The BackgroundColorValue describes the background color of a TIFF/IT BP or BL bitmap.
        ///
        /// references:  \
        /// <a href="#TIFFIT">TIFFIT</a>
        pub const BackgroundColorValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BackgroundColorValue",
            tag: 34026,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/IT BP/BL background color value",
            long_description: "The BackgroundColorValue describes the background color of a TIFF/IT BP or BL bitmap.",
            references: "<a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT MP pixel intensity range
        ///
        /// The PixelIntensityRange is similar to DotRange for a TIFF/IT MP image.
        ///
        /// references:  \
        /// <a href="#TIFFIT">TIFFIT</a>
        pub const PixelIntensityRange: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "PixelIntensityRange",
            tag: 34027,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/IT MP pixel intensity range",
            long_description: "The PixelIntensityRange is similar to DotRange for a TIFF/IT MP image.",
            references: "<a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT HC transparency indicator
        ///
        /// The TransparencyIndicator field denotes whether transparency information is within TIFF/IT HC data.
        ///
        /// references:  \
        /// <a href="#TIFFIT">TIFFIT</a>
        pub const TransparencyIndicator: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TransparencyIndicator",
            tag: 34028,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/IT HC transparency indicator",
            long_description: "The TransparencyIndicator field denotes whether transparency information is within TIFF/IT HC data.",
            references: "<a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT color characterization
        ///
        /// The ColorCharacterization fields describes colors per ISO 12641, ISO 12642, and ANSI CGATS.15.
        ///
        /// references:  \
        /// <a href="#TIFFIT">TIFFIT</a>
        pub const ColorCharacterization: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ColorCharacterization",
            tag: 34029,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/IT color characterization",
            long_description: "The ColorCharacterization fields describes colors per ISO 12641, ISO 12642, and ANSI CGATS.15.",
            references: "<a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// TIFF/IT HC usage
        ///
        /// The HCUsage field defines the type of information in the TIFF/IT HC file.
        ///
        /// references:  \
        /// <a href="#TIFFIT">TIFFIT</a>
        pub const HCUsage: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "HCUsage",
            tag: 34030,
            dtype: &[IfdValueType::Long, IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/IT HC usage",
            long_description: "The HCUsage field defines the type of information in the TIFF/IT HC file.",
            references: "<a href=\"#TIFFIT\">TIFFIT</a>",
        }
    ;

    
        /// IPTC/NAA metadata record
        ///
        /// The KodakIPTC field contains an IPTC/NAA record.
        ///
        /// references:  \
        /// <a href="#RICHTIFF">RICHTIFF</a>
        pub const KodakIPTC: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "KodakIPTC",
            tag: 34152,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "IPTC/NAA metadata record",
            long_description: "The KodakIPTC field contains an IPTC/NAA record.",
            references: "<a href=\"#RICHTIFF\">RICHTIFF</a>",
        }
    ;

    
        /// Pixel Magic JBIG options (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const PixelMagicJBIGOptions: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "PixelMagicJBIGOptions",
            tag: 34232,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Pixel Magic JBIG options (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// GeoTIFF model transformation
        ///
        /// 
        ///
        /// references:  \
        /// <a href="#GEOTIFF">GEOTIFF</a>
        pub const ModelTransformationTag: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ModelTransformationTag",
            tag: 34264,
            dtype: &[IfdValueType::Double, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(16),
            description: "GeoTIFF model transformation",
            long_description: "",
            references: "<a href=\"#GEOTIFF\">GEOTIFF</a>",
        }
    ;

    
        /// Adobe Photoshop image resource blocks
        ///
        /// The Photoshop field contains information embedded by the Adobe Photoshop application.
        ///
        /// references:  \
        /// <a href="#PSFF">PSFF</a>
        pub const ImageResourceBlocks: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageResourceBlocks",
            tag: 34377,
            dtype: &[IfdValueType::Undefined, IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Adobe Photoshop image resource blocks",
            long_description: "The Photoshop field contains information embedded by the Adobe Photoshop application.",
            references: "<a href=\"#PSFF\">PSFF</a>",
        }
    ;

    
        /// Exif IFD offset
        ///
        /// The ExifIFD field contains an offset to a sub-IFD containing Exif (digital camera) information.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>, p. 19
        pub const ExifIFD: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExifIFD",
            tag: 34665,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::IfdOffset { ifd_type: IfdType::Exif },
            count: IfdCount::ConcreteValue(1),
            description: "Exif IFD offset",
            long_description: "The ExifIFD field contains an offset to a sub-IFD containing Exif (digital camera) information.",
            references: "<a href=\"#EXIF21\">EXIF21</a>, p. 19",
        }
    ;

    
        /// ICC profile
        ///
        /// The InterColorProfile field contains an InterColor Consortium (ICC) format color space characterization/profile.
        ///
        /// references:  \
        /// <a href="#ICCEMBED">ICCEMBED</a> ///  <a href="#TIFFEP">TIFFEP</a>
        pub const InterColorProfile: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "InterColorProfile",
            tag: 34675,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "ICC profile",
            long_description: "The InterColorProfile field contains an InterColor Consortium (ICC) format color space characterization/profile.",
            references: "<a href=\"#ICCEMBED\">ICCEMBED</a> \n <a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF-FX Extensions
        ///
        /// The TIFF-FXExtensions field describes extensions used.
        ///
        /// references:  \
        /// See also<a href="#GlobalParametersIFD">GlobalParametersIFD</a>. <a href="#TIFFFXEX1">TIFFFXEX1</a>
        pub const TIFF_FXExtensions: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TIFF_FXExtensions",
            tag: 34687,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Bitflags { values: &[(0, "Resolution_ImageWidth (Extended resolutions and imagewidths extension)"), (1, "N_Layer_ProfileM (N-Layer Profile M extension)"), (2, "SharedData (Shared data extension)"), (3, "BilevelJBIG2_ProfileT (Black-and-White JBIG2 coding extension)"), (4, "JBIG2Extension_ProfileM (JBIG2 mask layer coding and foreground layer color tag extension)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF-FX Extensions",
            long_description: "The TIFF-FXExtensions field describes extensions used.",
            references: "See also<a href=\"#GlobalParametersIFD\">GlobalParametersIFD</a>. <a href=\"#TIFFFXEX1\">TIFFFXEX1</a>",
        }
    ;

    
        /// TIFF-FX multiple profiles
        ///
        /// The MultiProfiles field describes multiple profiles used.
        ///
        /// references:  \
        /// See also<a href="#GlobalParametersIFD">GlobalParametersIFD</a>, <a href="#FaxProfile">FaxProfile</a>, <a href="#TIFF-FXExtensions">TIFF-FXExtensions</a>. <a href="#TIFFFXEX1">TIFFFXEX1</a>
        pub const MultiProfiles: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "MultiProfiles",
            tag: 34688,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Bitflags { values: &[(0, "ProfileS"), (1, "ProfileF"), (2, "ProfileJ"), (3, "ProfileC"), (4, "ProfileL"), (5, "ProfileM"), (6, "ProfileT"), (7, "Resolution_ImageWidth (Extended resolutions and imagewidths extension)"), (8, "N_Layer_ProfileM (N-Layer Profile M extension)"), (9, "SharedData (Shared data extension)"), (10, "JBIG2Extension_ProfileM (JBIG2 mask layer coding and foreground layer color tag extension)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF-FX multiple profiles",
            long_description: "The MultiProfiles field describes multiple profiles used.",
            references: "See also<a href=\"#GlobalParametersIFD\">GlobalParametersIFD</a>, <a href=\"#FaxProfile\">FaxProfile</a>, <a href=\"#TIFF-FXExtensions\">TIFF-FXExtensions</a>. <a href=\"#TIFFFXEX1\">TIFFFXEX1</a>",
        }
    ;

    
        /// TIFF-FX shared data offset
        ///
        /// The SharedData field cotains an offset to the TIFF-FX Extension share data block within the file.
        ///
        /// references:  \
        /// See also<a href="#GlobalParametersIFD">GlobalParametersIFD</a>. <a href="#TIFFFXEX1">TIFFFXEX1</a>
        pub const SharedData: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SharedData",
            tag: 34689,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF-FX shared data offset",
            long_description: "The SharedData field cotains an offset to the TIFF-FX Extension share data block within the file.",
            references: "See also<a href=\"#GlobalParametersIFD\">GlobalParametersIFD</a>. <a href=\"#TIFFFXEX1\">TIFFFXEX1</a>",
        }
    ;

    
        /// TIFF-FX T.88 options
        ///
        /// The T88Options field contains options of the ITU-T T.88 (JBIG2) coding.
        ///
        /// references:  \
        /// <a href="#TIFFFXEX1">TIFFFXEX1</a>
        pub const T88Options: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "T88Options",
            tag: 34690,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Bitflags { values: &[] },
            count: IfdCount::N,
            description: "TIFF-FX T.88 options",
            long_description: "The T88Options field contains options of the ITU-T T.88 (JBIG2) coding.",
            references: "<a href=\"#TIFFFXEX1\">TIFFFXEX1</a>",
        }
    ;

    
        /// TIFF-FX MRC image layer
        ///
        /// The ImageLayer field contains two values, one to describe of which of the three TIFF-FX MRC layers this image component is a part, the second is the order in that the image component is to be composited.
        ///
        /// references:  \
        /// <a href="#TIFFFX">TIFFFX</a>
        pub const ImageLayer: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageLayer",
            tag: 34732,
            dtype: &[IfdValueType::Short, IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "TIFF-FX MRC image layer",
            long_description: "The ImageLayer field contains two values, one to describe of which of the three TIFF-FX MRC layers this image component is a part, the second is the order in that the image component is to be composited.",
            references: "<a href=\"#TIFFFX\">TIFFFX</a>",
        }
    ;

    
        /// GeoTIFF key directory
        ///
        /// Also called ProjectionInfoTag, CoordSystemInfoTag.
        ///
        /// references:  \
        /// <a href="#GEOTIFF">GEOTIFF</a>
        pub const GeoKeyDirectoryTag: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GeoKeyDirectoryTag",
            tag: 34735,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "GeoTIFF key directory",
            long_description: "Also called ProjectionInfoTag, CoordSystemInfoTag.",
            references: "<a href=\"#GEOTIFF\">GEOTIFF</a>",
        }
    ;

    
        /// GeoTIFF double parameters
        ///
        /// 
        ///
        /// references:  \
        /// <a href="#GEOTIFF">GEOTIFF</a>
        pub const GeoDoubleParamsTag: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GeoDoubleParamsTag",
            tag: 34736,
            dtype: &[IfdValueType::Double, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "GeoTIFF double parameters",
            long_description: "",
            references: "<a href=\"#GEOTIFF\">GEOTIFF</a>",
        }
    ;

    
        /// GeoTIFF ASCII parameters
        ///
        /// 
        ///
        /// references:  \
        /// <a href="#GEOTIFF">GEOTIFF</a>
        pub const GeoAsciiParamsTag: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GeoAsciiParamsTag",
            tag: 34737,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "GeoTIFF ASCII parameters",
            long_description: "",
            references: "<a href=\"#GEOTIFF\">GEOTIFF</a>",
        }
    ;

    
        /// TIFF/EP picture exposure program
        ///
        /// The ExposureProgram field describes the exposure setting program condition of the picture.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const ExposureProgram: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExposureProgram",
            tag: 34850,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Undefined"), (1, "Manual"), (2, "NormalProgram"), (3, "AperturePriority"), (4, "ShutterPriority"), (5, "CreativeProgram"), (6, "ActionProgram"), (7, "PortraitMode"), (8, "LandscapeMode"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture exposure program",
            long_description: "The ExposureProgram field describes the exposure setting program condition of the picture.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture spectral sensitivity
        ///
        /// The SpectralSensitivity field contains a description of the sensitivity of each channel of the image data according to ASTM standards.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const SpectralSensitivity: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SpectralSensitivity",
            tag: 34852,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture spectral sensitivity",
            long_description: "The SpectralSensitivity field contains a description of the sensitivity of each channel of the image data according to ASTM standards.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// Exif GPS offset
        ///
        /// The GPSInfoIFD field contains an offset to a sub-IFD containing GPS (Global Positioning System) information.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>, p. 19///  <a href="#TIFFEP">TIFFEP</a>
        pub const GPSInfoIFD: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSInfoIFD",
            tag: 34853,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::IfdOffset { ifd_type: IfdType::GpsInfo },
            count: IfdCount::ConcreteValue(1),
            description: "Exif GPS offset",
            long_description: "The GPSInfoIFD field contains an offset to a sub-IFD containing GPS (Global Positioning System) information.",
            references: "<a href=\"#EXIF21\">EXIF21</a>, p. 19\n <a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture ISO speed ratings
        ///
        /// The ISOSpeedRatings field contains the ISO speed or ISO latitude of the camera as specified by ISO 12232.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const ISOSpeedRatings: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ISOSpeedRatings",
            tag: 34855,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture ISO speed ratings",
            long_description: "The ISOSpeedRatings field contains the ISO speed or ISO latitude of the camera as specified by ISO 12232.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture optoelectronic conversion function
        ///
        /// The OECF field contains a specification of an opto-electronic conversion function as specified by ISO 14524.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const OECF: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "OECF",
            tag: 34856,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture optoelectronic conversion function",
            long_description: "The OECF field contains a specification of an opto-electronic conversion function as specified by ISO 14524.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP field number
        ///
        /// The Interlace field contains a value that describes the vertical and horizontal field of multiple field TIFF/EP images.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const Interlace: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Interlace",
            tag: 34857,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP field number",
            long_description: "The Interlace field contains a value that describes the vertical and horizontal field of multiple field TIFF/EP images.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP time zone offset
        ///
        /// The TimeZoneOffset contains the time zone offset in hours from GMT for the DateTimeOriginal field and optionally the DateTime field of the TIFF/EP image.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const TimeZoneOffset: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TimeZoneOffset",
            tag: 34858,
            dtype: &[IfdValueType::SShort, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP time zone offset",
            long_description: "The TimeZoneOffset contains the time zone offset in hours from GMT for the DateTimeOriginal field and optionally the DateTime field of the TIFF/EP image.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP self timer mode
        ///
        /// The SelfTimerMode field contains the number of seconds from when the plunger was depressed that the camera fired, or zero for no delay.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const SelfTimerMode: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SelfTimerMode",
            tag: 34859,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP self timer mode",
            long_description: "The SelfTimerMode field contains the number of seconds from when the plunger was depressed that the camera fired, or zero for no delay.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// SGI fax receival parameters (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const FaxRecvParams: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FaxRecvParams",
            tag: 34908,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "SGI fax receival parameters (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// SGI fax subaddress (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const FaxSubAddress: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FaxSubAddress",
            tag: 34909,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "SGI fax subaddress (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// SGI fax receival time (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const FaxRecvTime: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FaxRecvTime",
            tag: 34910,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "SGI fax receival time (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// TIFF/EP origination date/time
        ///
        /// The DateTimeOriginal field contains 20 ASCII characters in the form "YYYY:MM:DD HH:MM:SS" indicating the data and time when the image data was sampled.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const DateTimeOriginal: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DateTimeOriginal",
            tag: 36867,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(20),
            description: "TIFF/EP origination date/time",
            long_description: "The DateTimeOriginal field contains 20 ASCII characters in the form \"YYYY:MM:DD HH:MM:SS\" indicating the data and time when the image data was sampled.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP compressed bits per pixel
        ///
        /// The CompressedBitsPerPixel field contains TIFF/EP data compression information.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const CompressedBitsPerPixel: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CompressedBitsPerPixel",
            tag: 37122,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP compressed bits per pixel",
            long_description: "The CompressedBitsPerPixel field contains TIFF/EP data compression information.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP shutter speed
        ///
        /// The ShutterSpeedValue contains the shutter speed in APEX units of the TIFF/EP image.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const ShutterSpeedValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ShutterSpeedValue",
            tag: 37377,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP shutter speed",
            long_description: "The ShutterSpeedValue contains the shutter speed in APEX units of the TIFF/EP image.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture lens aperture
        ///
        /// The ApertureValue field contains the APEX unit valued lens aperture.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const ApertureValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ApertureValue",
            tag: 37378,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture lens aperture",
            long_description: "The ApertureValue field contains the APEX unit valued lens aperture.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture brightness
        ///
        /// The BrightnessValue field contains the APEX unit valued brightness.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const BrightnessValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BrightnessValue",
            tag: 37379,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture brightness",
            long_description: "The BrightnessValue field contains the APEX unit valued brightness.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture exposure bias
        ///
        /// The ExposureBiasValue field contains the APEX unit valued exposure bias.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const ExposureBiasValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExposureBiasValue",
            tag: 37380,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture exposure bias",
            long_description: "The ExposureBiasValue field contains the APEX unit valued exposure bias.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture lens maximum aperture
        ///
        /// The MaxApertureValue field contains the APEX unit valued minimum F number of the lens.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const MaxApertureValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "MaxApertureValue",
            tag: 37381,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture lens maximum aperture",
            long_description: "The MaxApertureValue field contains the APEX unit valued minimum F number of the lens.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture subject distance
        ///
        /// The SubjectDistance field contains the distance from the camera to the picture's subject, in meters.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const SubjectDistance: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubjectDistance",
            tag: 37382,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture subject distance",
            long_description: "The SubjectDistance field contains the distance from the camera to the picture's subject, in meters.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture metering mode
        ///
        /// The MeteringMode field describes the metering mode of the camera.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const MeteringMode: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "MeteringMode",
            tag: 37383,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Unidentified"), (1, "Average"), (2, "CenterWeightedAverage"), (3, "Spot"), (4, "MultiSpot"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture metering mode",
            long_description: "The MeteringMode field describes the metering mode of the camera.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture light source
        ///
        /// The LightSource field describes the light source conditions of the picture.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const LightSource: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "LightSource",
            tag: 37384,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Unidentified"), (1, "Daylight"), (2, "Fluorescent"), (3, "Tungsten"), (10, "Flash"), (17, "StandardIlluminantA"), (18, "StandardIlluminantB"), (19, "StandardIlluminantC"), (20, "D55Illuminant"), (21, "D65Illuminant"), (22, "D75Illuminant"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture light source",
            long_description: "The LightSource field describes the light source conditions of the picture.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture flash usage
        ///
        /// The Flash field describes the use of strobe flash with the picture.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const Flash: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Flash",
            tag: 37385,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture flash usage",
            long_description: "The Flash field describes the use of strobe flash with the picture.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture lens focal length
        ///
        /// The FocalLength field contains the focal length in millimeters of the lens.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const FocalLength: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FocalLength",
            tag: 37386,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture lens focal length",
            long_description: "The FocalLength field contains the focal length in millimeters of the lens.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture flash energy
        ///
        /// The FlashEnergy field contains the power of the strobe flash in BCPS, beam candlepower seconds, units.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const FlashEnergy: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FlashEnergy",
            tag: 37387,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture flash energy",
            long_description: "The FlashEnergy field contains the power of the strobe flash in BCPS, beam candlepower seconds, units.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture spatial frequency response
        ///
        /// The SpatialFrequencyResponse field contains the device spatial frequency response table and values for the picture per ISO 12233.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const SpatialFrequencyResponse: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SpatialFrequencyResponse",
            tag: 37388,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture spatial frequency response",
            long_description: "The SpatialFrequencyResponse field contains the device spatial frequency response table and values for the picture per ISO 12233.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP noise measurement
        ///
        /// The Noise field contains a measurement of the noise value of the TIFF/EP image.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const Noise: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Noise",
            tag: 37389,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP noise measurement",
            long_description: "The Noise field contains a measurement of the noise value of the TIFF/EP image.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture focal plane column resolution
        ///
        /// The FocalPlaneXResolution field contains the focal plane column resolution in FocalPlaneResolutionUnits units.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const FocalPlaneXResolution: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FocalPlaneXResolution",
            tag: 37390,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture focal plane column resolution",
            long_description: "The FocalPlaneXResolution field contains the focal plane column resolution in FocalPlaneResolutionUnits units.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture focal plane row resolution
        ///
        /// The FocalPlaneXResolution field contains the focal plane row resolution in FocalPlaneResolutionUnit units.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const FocalPlaneYResolution: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FocalPlaneYResolution",
            tag: 37391,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture focal plane row resolution",
            long_description: "The FocalPlaneXResolution field contains the focal plane row resolution in FocalPlaneResolutionUnit units.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture focal plane resolution unit
        ///
        /// The FocalPlaneResolutionUnit field describes the focal plane resolution unit.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const FocalPlaneResolutionUnit: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FocalPlaneResolutionUnit",
            tag: 37392,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "Inch"), (2, "Meter"), (3, "Centimeter"), (4, "Millimeter"), (5, "Micrometer"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture focal plane resolution unit",
            long_description: "The FocalPlaneResolutionUnit field describes the focal plane resolution unit.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP image number
        ///
        /// The ImageNumber fields contains an identifier assigned to an image in a TIFF/EP file.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const ImageNumber: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageNumber",
            tag: 37393,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP image number",
            long_description: "The ImageNumber fields contains an identifier assigned to an image in a TIFF/EP file.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP security classification
        ///
        /// The SecurityClassification field contains either a single ASCII character or an ASCII string describing the security classification of the image per the NITF specification (MIL-STD-2500).
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const SecurityClassification: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SecurityClassification",
            tag: 37394,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP security classification",
            long_description: "The SecurityClassification field contains either a single ASCII character or an ASCII string describing the security classification of the image per the NITF specification (MIL-STD-2500).",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP image modification history
        ///
        /// The ImageHistory field contains a description of modifications to the TIFF/EP image.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const ImageHistory: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageHistory",
            tag: 37395,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP image modification history",
            long_description: "The ImageHistory field contains a description of modifications to the TIFF/EP image.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture subject location
        ///
        /// The SubjectLocation field contains two coordinate values into the image of the pixel of the subject location in the picture.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const SubjectLocation: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubjectLocation",
            tag: 37396,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture subject location",
            long_description: "The SubjectLocation field contains two coordinate values into the image of the pixel of the subject location in the picture.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture exposure index
        ///
        /// The ExposureIndex field contains the camera exposure index setting.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const ExposureIndex: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExposureIndex",
            tag: 37397,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "TIFF/EP picture exposure index",
            long_description: "The ExposureIndex field contains the camera exposure index setting.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP standard identifier
        ///
        /// The TIFFEPStandardID field contains four ASCII characters representing the TIFF/EP standard version of a TIFF/EP file, eg '1', '0', '0', '0'.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const TIFFEPStandardID: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TIFFEPStandardID",
            tag: 37398,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(4),
            description: "TIFF/EP standard identifier",
            long_description: "The TIFFEPStandardID field contains four ASCII characters representing the TIFF/EP standard version of a TIFF/EP file, eg '1', '0', '0', '0'.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// TIFF/EP picture sensing method
        ///
        /// The SensingMethod field describes the sensors that capture the image of the picture.
        ///
        /// references:  \
        /// <a href="#TIFFEP">TIFFEP</a>
        pub const SensingMethod: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SensingMethod",
            tag: 37399,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Undefined"), (1, "MonochromeArea"), (2, "OneChipColorArea"), (3, "TwoChipColorArea"), (4, "ThreeChipColorArea"), (5, "ColorSequentialArea"), (6, "MonochromeLinearArea"), (7, "TriLinear"), (8, "ColorSequentialLinear"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "TIFF/EP picture sensing method",
            long_description: "The SensingMethod field describes the sensors that capture the image of the picture.",
            references: "<a href=\"#TIFFEP\">TIFFEP</a>",
        }
    ;

    
        /// CIP3 PPF data
        ///
        /// The CIP3DataFile field contains a string that is to be intepreted as the filename of a CIP3 PPF file, as a field of a TIFF/IT FP IFD.
        ///
        /// references:  \
        /// <a href="#CIP3EMBED">CIP3EMBED</a>
        pub const CIP3DataFile: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CIP3DataFile",
            tag: 37434,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "CIP3 PPF data",
            long_description: "The CIP3DataFile field contains a string that is to be intepreted as the filename of a CIP3 PPF file, as a field of a TIFF/IT FP IFD.",
            references: "<a href=\"#CIP3EMBED\">CIP3EMBED</a>",
        }
    ;

    
        /// CIP3 sheet name
        ///
        /// The CIP3Sheet field contains a string that references the sheet to use in a multiple sheet PPF file, as a field of a TIFF/IT FP IFD.
        ///
        /// references:  \
        /// <a href="#CIP3EMBED">CIP3EMBED</a>
        pub const CIP3Sheet: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CIP3Sheet",
            tag: 37435,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "CIP3 sheet name",
            long_description: "The CIP3Sheet field contains a string that references the sheet to use in a multiple sheet PPF file, as a field of a TIFF/IT FP IFD.",
            references: "<a href=\"#CIP3EMBED\">CIP3EMBED</a>",
        }
    ;

    
        /// CIP3 sheet side
        ///
        /// The CIP3Side field describes which side of a PPF sheet is to be used, as a field of a TIFF/IT FP IFD.
        ///
        /// references:  \
        /// <a href="#CIP3EMBED">CIP3EMBED</a>
        pub const CIP3Side: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CIP3Side",
            tag: 37436,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "CIP3 sheet side",
            long_description: "The CIP3Side field describes which side of a PPF sheet is to be used, as a field of a TIFF/IT FP IFD.",
            references: "<a href=\"#CIP3EMBED\">CIP3EMBED</a>",
        }
    ;

    
        /// Adobe Photoshop image source data
        ///
        /// The ImageSourceData field contains information embedded by the Adobe Photoshop application.
        ///
        /// references:  \
        /// <a href="#TIFFPS">TIFFPS</a>
        pub const ImageSourceData: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageSourceData",
            tag: 37724,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Adobe Photoshop image source data",
            long_description: "The ImageSourceData field contains information embedded by the Adobe Photoshop application.",
            references: "<a href=\"#TIFFPS\">TIFFPS</a>",
        }
    ;

    
        /// GDAL metadata (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const GDAL_METADATA: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GDAL_METADATA",
            tag: 42112,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "GDAL metadata (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// GDAL background/nodata (private tag)
        ///
        /// 
        ///
        /// references:  \
        /// 
        pub const GDAL_NODATA: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GDAL_NODATA",
            tag: 42113,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "GDAL background/nodata (private tag)",
            long_description: "",
            references: "",
        }
    ;

    
        /// USPTO Original Content Type (private tag)
        ///
        /// The USPTO OriginalContentType field describes the original content type of the image.
        ///
        /// references:  \
        /// <a href="#YB2">YB2</a>, p. 7
        pub const USPTOOriginalContentType: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "USPTOOriginalContentType",
            tag: 50560,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "TextOrDrawing (Text or black and white drawing (default))"), (1, "Grayscale (Grayscale drawing or photograph)"), (2, "Color (Color drawing or photograph)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "USPTO Original Content Type (private tag)",
            long_description: "The USPTO OriginalContentType field describes the original content type of the image.",
            references: "<a href=\"#YB2\">YB2</a>, p. 7",
        }
    ;

    
        /// DNG version
        ///
        /// The DNGVersion contains four bytes containing the numeric value of the file's conformance version level to the DNG (Digital Negative) specification.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 15
        pub const DNGVersion: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DNGVersion",
            tag: 50706,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(4),
            description: "DNG version",
            long_description: "The DNGVersion contains four bytes containing the numeric value of the file's conformance version level to the DNG (Digital Negative) specification.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 15",
        }
    ;

    
        /// DNG backwards compatible version
        ///
        /// The DNGBackwardsVersion field contains four bytes containing the numeric value of the file's conformance version level to a DNG (Digital Negative) specification.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 15
        pub const DNGBackwardVersion: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DNGBackwardVersion",
            tag: 50707,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(4),
            description: "DNG backwards compatible version",
            long_description: "The DNGBackwardsVersion field contains four bytes containing the numeric value of the file's conformance version level to a DNG (Digital Negative) specification.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 15",
        }
    ;

    
        /// DNG unique camera model
        ///
        /// The UniqueCameraModel field contains a null-terminated ASCII string noting the camera model.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 16
        pub const UniqueCameraModel: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "UniqueCameraModel",
            tag: 50708,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG unique camera model",
            long_description: "The UniqueCameraModel field contains a null-terminated ASCII string noting the camera model.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 16",
        }
    ;

    
        /// DNG localized camera model
        ///
        /// The LocalizedCameraModel field contains a null-terminated ASCII string or a Unicode string noting the camera model.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 17
        pub const LocalizedCameraModel: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "LocalizedCameraModel",
            tag: 50709,
            dtype: &[IfdValueType::Ascii, IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG localized camera model",
            long_description: "The LocalizedCameraModel field contains a null-terminated ASCII string or a Unicode string noting the camera model.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 17",
        }
    ;

    
        /// DNG CFA plane color
        ///
        /// The CFAPlaneColor fields contains a list of zero-based digits indicating the order of the color planes of the color filter array pattern for the LinearRaw photometric interpretation.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 17
        pub const CFAPlaneColor: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CFAPlaneColor",
            tag: 50710,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG CFA plane color",
            long_description: "The CFAPlaneColor fields contains a list of zero-based digits indicating the order of the color planes of the color filter array pattern for the LinearRaw photometric interpretation.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 17",
        }
    ;

    
        /// DNG CFA spatial layout
        ///
        /// The CFALayout field denotes the spatial layout of the color filter array.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 18
        pub const CFALayout: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CFALayout",
            tag: 50711,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "Rectangular"), (2, "Staggered_A"), (3, "Staggered_B"), (4, "Staggered_C"), (5, "Staggered_D"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "DNG CFA spatial layout",
            long_description: "The CFALayout field denotes the spatial layout of the color filter array.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 18",
        }
    ;

    
        /// DNG linearization table
        ///
        /// The LinearizationTable field contains a lookup table (LUT) that maps data values of the samples of the image to non-linear values.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 18
        pub const LinearizationTable: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "LinearizationTable",
            tag: 50712,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG linearization table",
            long_description: "The LinearizationTable field contains a lookup table (LUT) that maps data values of the samples of the image to non-linear values.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 18",
        }
    ;

    
        /// DNG black level repeat dimensions
        ///
        /// The BlackLevelRepeatDim field contains two values, one each for rows and columns of the black level tag.
        ///
        /// references:  \
        /// See also<a href="#BlackLevel">BlackLevel</a>. <a href="#DNG1000">DNG1000</a>, p. 19
        pub const BlackLevelRepeatDim: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BlackLevelRepeatDim",
            tag: 50713,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "DNG black level repeat dimensions",
            long_description: "The BlackLevelRepeatDim field contains two values, one each for rows and columns of the black level tag.",
            references: "See also<a href=\"#BlackLevel\">BlackLevel</a>. <a href=\"#DNG1000\">DNG1000</a>, p. 19",
        }
    ;

    
        /// DNG black level
        ///
        /// The BlackLevel field contains the "zero light" or thermal black encoding level, as a repeating pattern. The values are stored in row-column-sample scan order.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 19
        pub const BlackLevel: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BlackLevel",
            tag: 50714,
            dtype: &[IfdValueType::Short, IfdValueType::Long, IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG black level",
            long_description: "The BlackLevel field contains the \"zero light\" or thermal black encoding level, as a repeating pattern. The values are stored in row-column-sample scan order.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 19",
        }
    ;

    
        /// DNG black level delta - horizontal
        ///
        /// The BlackLevelDeltaH field encodes the per-column difference of the "zero light" level.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 20
        pub const BlackLevelDeltaH: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BlackLevelDeltaH",
            tag: 50715,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG black level delta - horizontal",
            long_description: "The BlackLevelDeltaH field encodes the per-column difference of the \"zero light\" level.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 20",
        }
    ;

    
        /// DNG black level delta - vertical
        ///
        /// The BlackLevelDeltaV field encodes the per-row difference of the "zero light" level.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 20
        pub const BlackLevelDeltaV: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BlackLevelDeltaV",
            tag: 50716,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG black level delta - vertical",
            long_description: "The BlackLevelDeltaV field encodes the per-row difference of the \"zero light\" level.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 20",
        }
    ;

    
        /// DNG white level
        ///
        /// The WhiteLevel field contains the fully-saturated encoding level for the raw samples, per sample.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 21
        pub const WhiteLevel: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "WhiteLevel",
            tag: 50717,
            dtype: &[IfdValueType::Short, IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG white level",
            long_description: "The WhiteLevel field contains the fully-saturated encoding level for the raw samples, per sample.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 21",
        }
    ;

    
        /// DNG default scale
        ///
        /// The DefaultScale field contains a pair of scale factors for cameras with non-square pixels.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 21
        pub const DefaultScale: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DefaultScale",
            tag: 50718,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "DNG default scale",
            long_description: "The DefaultScale field contains a pair of scale factors for cameras with non-square pixels.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 21",
        }
    ;

    
        /// DNG default crop origin
        ///
        /// The DefaultCropOrigin field contains a pair of coordinates the mark the origin, in raw image coordinates.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 22
        pub const DefaultCropOrigin: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DefaultCropOrigin",
            tag: 50719,
            dtype: &[IfdValueType::Short, IfdValueType::Long, IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "DNG default crop origin",
            long_description: "The DefaultCropOrigin field contains a pair of coordinates the mark the origin, in raw image coordinates.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 22",
        }
    ;

    
        /// DNG default crop size
        ///
        /// The DefaultCropSize field contains a pair of coordinates that mark the extent, in raw image coordinates.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 23
        pub const DefaultCropSize: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DefaultCropSize",
            tag: 50720,
            dtype: &[IfdValueType::Short, IfdValueType::Long, IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "DNG default crop size",
            long_description: "The DefaultCropSize field contains a pair of coordinates that mark the extent, in raw image coordinates.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 23",
        }
    ;

    
        /// DNG color matrix, set one
        ///
        /// The ColorMatrix1 field contains a transformation matrix to convert CIE XYZ values to reference camera native color space values, under the illuminant specified as CalibrationIlluminant1. The matrix values are stored in row scan order.
        ///
        /// references:  \
        /// See also<a href="#CalibrationIlluminant1">CalibrationIlluminant1</a>. <a href="#DNG1000">DNG1000</a>, p. 24
        pub const ColorMatrix1: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ColorMatrix1",
            tag: 50721,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG color matrix, set one",
            long_description: "The ColorMatrix1 field contains a transformation matrix to convert CIE XYZ values to reference camera native color space values, under the illuminant specified as CalibrationIlluminant1. The matrix values are stored in row scan order.",
            references: "See also<a href=\"#CalibrationIlluminant1\">CalibrationIlluminant1</a>. <a href=\"#DNG1000\">DNG1000</a>, p. 24",
        }
    ;

    
        /// DNG color matrix, set two
        ///
        /// The ColorMatrix2 field contains a transformation matrix to convert CIE XYZ values to reference camera native color space values, under the illuminant specified as CalibrationIlluminant2. The matrix values are stored in row scan order.
        ///
        /// references:  \
        /// See also<a href="#CalibrationIlluminant2">CalibrationIlluminant2</a>. <a href="#DNG1000">DNG1000</a>, p. 25
        pub const ColorMatrix2: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ColorMatrix2",
            tag: 50722,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG color matrix, set two",
            long_description: "The ColorMatrix2 field contains a transformation matrix to convert CIE XYZ values to reference camera native color space values, under the illuminant specified as CalibrationIlluminant2. The matrix values are stored in row scan order.",
            references: "See also<a href=\"#CalibrationIlluminant2\">CalibrationIlluminant2</a>. <a href=\"#DNG1000\">DNG1000</a>, p. 25",
        }
    ;

    
        /// DNG camera calibration, set one
        ///
        /// The CameraCalibration1 field contains a transformation matrix to convert reference camera native color space values to individual camera native color space samples, under the illuminant specified as CalibrationIlluminant1. The matrix is stored in row scan order.
        ///
        /// references:  \
        /// See also<a href="#CalibrationIlluminant1">CalibrationIlluminant1</a>. <a href="#DNG1000">DNG1000</a>, p. 25
        pub const CameraCalibration1: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CameraCalibration1",
            tag: 50723,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG camera calibration, set one",
            long_description: "The CameraCalibration1 field contains a transformation matrix to convert reference camera native color space values to individual camera native color space samples, under the illuminant specified as CalibrationIlluminant1. The matrix is stored in row scan order.",
            references: "See also<a href=\"#CalibrationIlluminant1\">CalibrationIlluminant1</a>. <a href=\"#DNG1000\">DNG1000</a>, p. 25",
        }
    ;

    
        /// DNG camera calibration, set two
        ///
        /// The CameraCalibration2 field contains a transformation matrix to convert reference camera native color space values to individual camera native color space samples, under the illuminant specified as CalibrationIlluminant2. The matrix is stored in row scan order.
        ///
        /// references:  \
        /// See also<a href="#CalibrationIlluminant2">CalibrationIlluminant2</a>. <a href="#DNG1000">DNG1000</a>, p. 26
        pub const CameraCalibration2: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CameraCalibration2",
            tag: 50724,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG camera calibration, set two",
            long_description: "The CameraCalibration2 field contains a transformation matrix to convert reference camera native color space values to individual camera native color space samples, under the illuminant specified as CalibrationIlluminant2. The matrix is stored in row scan order.",
            references: "See also<a href=\"#CalibrationIlluminant2\">CalibrationIlluminant2</a>. <a href=\"#DNG1000\">DNG1000</a>, p. 26",
        }
    ;

    
        /// DNG reduction matrix, set one
        ///
        /// The ReductionMatrix1 contains a dimensionality reduction matrix for use as the first stage of converting camera native color space values to CIE XYZ, under the illuminant specified as CalibrationIlluminant1.
        ///
        /// references:  \
        /// See also<a href="#CalibrationIlluminant1">CalibrationIlluminant1</a>. <a href="#DNG1000">DNG1000</a>, p. 27
        pub const ReductionMatrix1: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ReductionMatrix1",
            tag: 50725,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG reduction matrix, set one",
            long_description: "The ReductionMatrix1 contains a dimensionality reduction matrix for use as the first stage of converting camera native color space values to CIE XYZ, under the illuminant specified as CalibrationIlluminant1.",
            references: "See also<a href=\"#CalibrationIlluminant1\">CalibrationIlluminant1</a>. <a href=\"#DNG1000\">DNG1000</a>, p. 27",
        }
    ;

    
        /// DNG reduction matrix, set two
        ///
        /// The ReductionMatrix2 contains a dimensionality reduction matrix for use as the first stage of converting camera native color space values to CIE XYZ, under the illuminant specified as CalibrationIlluminant2.
        ///
        /// references:  \
        /// See also<a href="#CalibrationIlluminant2">CalibrationIlluminant2</a>. <a href="#DNG1000">DNG1000</a>, p. 27
        pub const ReductionMatrix2: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ReductionMatrix2",
            tag: 50726,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG reduction matrix, set two",
            long_description: "The ReductionMatrix2 contains a dimensionality reduction matrix for use as the first stage of converting camera native color space values to CIE XYZ, under the illuminant specified as CalibrationIlluminant2.",
            references: "See also<a href=\"#CalibrationIlluminant2\">CalibrationIlluminant2</a>. <a href=\"#DNG1000\">DNG1000</a>, p. 27",
        }
    ;

    
        /// DNG analog balance
        ///
        /// The AnalogBalance field contains the gain values applied to white balance.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 28
        pub const AnalogBalance: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "AnalogBalance",
            tag: 50727,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG analog balance",
            long_description: "The AnalogBalance field contains the gain values applied to white balance.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 28",
        }
    ;

    
        /// DNG neutral white balance value
        ///
        /// The AsShotNeutral fields contains the neutral white balance color in linear reference color space values.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 28
        pub const AsShotNeutral: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "AsShotNeutral",
            tag: 50728,
            dtype: &[IfdValueType::Short, IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG neutral white balance value",
            long_description: "The AsShotNeutral fields contains the neutral white balance color in linear reference color space values.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 28",
        }
    ;

    
        /// DNG selected white balance
        ///
        /// The AsShotWhiteXY field contains xy chromaticity coordinates of the white balance.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 29
        pub const AsShotWhiteXY: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "AsShotWhiteXY",
            tag: 50729,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "DNG selected white balance",
            long_description: "The AsShotWhiteXY field contains xy chromaticity coordinates of the white balance.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 29",
        }
    ;

    
        /// DNG baseline exposure
        ///
        /// The BaselineExposure field contains the zero point for footroom, in EV units.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 29
        pub const BaselineExposure: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BaselineExposure",
            tag: 50730,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "DNG baseline exposure",
            long_description: "The BaselineExposure field contains the zero point for footroom, in EV units.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 29",
        }
    ;

    
        /// DNG baseline noise
        ///
        /// The BaselineNoise fields contains the relative noise of a camera at ISO 100 compared to the noise of a reference camera model.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 30
        pub const BaselineNoise: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BaselineNoise",
            tag: 50731,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "DNG baseline noise",
            long_description: "The BaselineNoise fields contains the relative noise of a camera at ISO 100 compared to the noise of a reference camera model.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 30",
        }
    ;

    
        /// DNG baseline sharpness
        ///
        /// The BaselineSharpness field contains the relative sharpening required for the camera model, compared to that of a reference camera model.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 30
        pub const BaselineSharpness: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BaselineSharpness",
            tag: 50732,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "DNG baseline sharpness",
            long_description: "The BaselineSharpness field contains the relative sharpening required for the camera model, compared to that of a reference camera model.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 30",
        }
    ;

    
        /// DNG Bayer green split
        ///
        /// The BayerGreenSplit field contains a value in arbitrary units relating the tracking of the green pixels of the blue/green rows to those in red/green rows, only in color filter arrays using a Bayer pattern filter array.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 31
        pub const BayerGreenSplit: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BayerGreenSplit",
            tag: 50733,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "DNG Bayer green split",
            long_description: "The BayerGreenSplit field contains a value in arbitrary units relating the tracking of the green pixels of the blue/green rows to those in red/green rows, only in color filter arrays using a Bayer pattern filter array.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 31",
        }
    ;

    
        /// DNG linear response limit
        ///
        /// The LinearResponseLimit field specifies the range of linear sensor response.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 31
        pub const LinearResponseLimit: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "LinearResponseLimit",
            tag: 50734,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "DNG linear response limit",
            long_description: "The LinearResponseLimit field specifies the range of linear sensor response.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 31",
        }
    ;

    
        /// DNG camera serial number
        ///
        /// The CameraSerialNumber contains the serial number of the camera or camera body.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 32
        pub const CameraSerialNumber: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CameraSerialNumber",
            tag: 50735,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "DNG camera serial number",
            long_description: "The CameraSerialNumber contains the serial number of the camera or camera body.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 32",
        }
    ;

    
        /// DNG lens information
        ///
        /// The LensInfo field contains values describing the focal length and F-stop of the lens used. The first and second values specify minimum and maximum focal length in millimeters, the third and fourth values specify minimum and maximum F-stop at minimum and maximum focal length and maximum and minimum aperture.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 32
        pub const LensInfo: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "LensInfo",
            tag: 50736,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(4),
            description: "DNG lens information",
            long_description: "The LensInfo field contains values describing the focal length and F-stop of the lens used. The first and second values specify minimum and maximum focal length in millimeters, the third and fourth values specify minimum and maximum F-stop at minimum and maximum focal length and maximum and minimum aperture.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 32",
        }
    ;

    
        /// DNG chroma blur radius
        ///
        /// The ChromaBlurRadius field contains a value specifying the chroma blur area.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 33
        pub const ChromaBlurRadius: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ChromaBlurRadius",
            tag: 50737,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "DNG chroma blur radius",
            long_description: "The ChromaBlurRadius field contains a value specifying the chroma blur area.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 33",
        }
    ;

    
        /// DNG anti-alias strength
        ///
        /// The AntiAliasStrength field denotes the relative strength of the camera's anti-alias filter, from 0.0 (no anti-aliasing filter) to 1.0 (effective anti-alias filter).
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 33
        pub const AntiAliasStrength: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "AntiAliasStrength",
            tag: 50738,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "DNG anti-alias strength",
            long_description: "The AntiAliasStrength field denotes the relative strength of the camera's anti-alias filter, from 0.0 (no anti-aliasing filter) to 1.0 (effective anti-alias filter).",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 33",
        }
    ;

    
        /// DNG private data field
        ///
        /// The DNGPrivateData field contains private data.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 34
        pub const DNGPrivateData: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DNGPrivateData",
            tag: 50740,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Blob,
            count: IfdCount::N,
            description: "DNG private data field",
            long_description: "The DNGPrivateData field contains private data.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 34",
        }
    ;

    
        /// DNG makernote safety
        ///
        /// The MakerNoteSafety field denotes whether it is safe to copy the meaningful contents of the MakerNote in editing the file.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 35
        pub const MakerNoteSafety: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "MakerNoteSafety",
            tag: 50741,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Unsafe"), (1, "Safe"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "DNG makernote safety",
            long_description: "The MakerNoteSafety field denotes whether it is safe to copy the meaningful contents of the MakerNote in editing the file.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 35",
        }
    ;

    
        /// DNG calibration illuminant, set one
        ///
        /// The CalibrationIlluminantField1 field denotes the light source for the first calibration set.
        ///
        /// references:  \
        /// See also<a href="#LightSource">LightSource</a>, <a href="#ColorMatrix1">ColorMatrix1</a>, <a href="#CameraCalibration1">CameraCalibration1</a>, <a href="#ReductionMatrix1">ReductionMatrix1</a>. <a href="#DNG1000">DNG1000</a>, p. 23
        pub const CalibrationIlluminant1: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CalibrationIlluminant1",
            tag: 50778,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Unidentified"), (1, "Daylight"), (2, "Fluorescent"), (3, "Tungsten"), (4, "Flash"), (9, "FineWeather"), (10, "CloudyWeather"), (11, "Shady"), (12, "DaylightFluorescent"), (13, "DayWhiteFluorescent"), (14, "CoolWhiteFluorescent"), (15, "WhiteFluorescent"), (17, "StandardIlluminantA"), (18, "StandardIlluminantB"), (19, "StandardIlluminantC"), (20, "D55Illuminant"), (21, "D65Illuminant"), (22, "D75Illuminant"), (23, "D50Illuminant"), (24, "ISOStudioTungsten"), (255, "Other"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "DNG calibration illuminant, set one",
            long_description: "The CalibrationIlluminantField1 field denotes the light source for the first calibration set.",
            references: "See also<a href=\"#LightSource\">LightSource</a>, <a href=\"#ColorMatrix1\">ColorMatrix1</a>, <a href=\"#CameraCalibration1\">CameraCalibration1</a>, <a href=\"#ReductionMatrix1\">ReductionMatrix1</a>. <a href=\"#DNG1000\">DNG1000</a>, p. 23",
        }
    ;

    
        /// DNG calibration illuminant, set two
        ///
        /// The CalibrationIlluminantField1 field denotes the light source for the second calibration set.
        ///
        /// references:  \
        /// See also<a href="#LightSource">LightSource</a>, <a href="#ColorMatrix2">ColorMatrix2</a>, <a href="#CameraCalibration2">CameraCalibration2</a>, <a href="#ReductionMatrix2">ReductionMatrix2</a>. <a href="#DNG1000">DNG1000</a>, p. 24
        pub const CalibrationIlluminant2: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CalibrationIlluminant2",
            tag: 50779,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Unidentified"), (1, "Daylight"), (2, "Fluorescent"), (3, "Tungsten"), (4, "Flash"), (9, "FineWeather"), (10, "CloudyWeather"), (11, "Shady"), (12, "DaylightFluorescent"), (13, "DayWhiteFluorescent"), (14, "CoolWhiteFluorescent"), (15, "WhiteFluorescent"), (17, "StandardIlluminantA"), (18, "StandardIlluminantB"), (19, "StandardIlluminantC"), (20, "D55Illuminant"), (21, "D65Illuminant"), (22, "D75Illuminant"), (23, "D50Illuminant"), (24, "ISOStudioTungsten"), (255, "Other"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "DNG calibration illuminant, set two",
            long_description: "The CalibrationIlluminantField1 field denotes the light source for the second calibration set.",
            references: "See also<a href=\"#LightSource\">LightSource</a>, <a href=\"#ColorMatrix2\">ColorMatrix2</a>, <a href=\"#CameraCalibration2\">CameraCalibration2</a>, <a href=\"#ReductionMatrix2\">ReductionMatrix2</a>. <a href=\"#DNG1000\">DNG1000</a>, p. 24",
        }
    ;

    
        /// DNG best-quality scale factor
        ///
        /// The BestQualityScale field contains a value to scale the default scale factors for improved quality.
        ///
        /// references:  \
        /// <a href="#DNG1000">DNG1000</a>, p. 22
        pub const BestQualityScale: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BestQualityScale",
            tag: 50780,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "DNG best-quality scale factor",
            long_description: "The BestQualityScale field contains a value to scale the default scale factors for improved quality.",
            references: "<a href=\"#DNG1000\">DNG1000</a>, p. 22",
        }
    ;

    
        /// Alias/Wavefront layer metadata (private tag)
        ///
        /// The AliasLayerMetadata field contains information per the Alias Systems Multi-Layer TIFF specification.
        ///
        /// references:  \
        /// See also<a href="#Software">Software</a>, <a href="#HostComputer">HostComputer</a>, <a href="#PageName">PageName</a>, <a href="#XPosition">XPosition</a>, <a href="#YPosition">YPosition</a>.
        pub const AliasLayerMetadata: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "AliasLayerMetadata",
            tag: 50784,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Alias/Wavefront layer metadata (private tag)",
            long_description: "The AliasLayerMetadata field contains information per the Alias Systems Multi-Layer TIFF specification.",
            references: "See also<a href=\"#Software\">Software</a>, <a href=\"#HostComputer\">HostComputer</a>, <a href=\"#PageName\">PageName</a>, <a href=\"#XPosition\">XPosition</a>, <a href=\"#YPosition\">YPosition</a>.",
        }
    ;

    
        /// Time Codes of the Image
        ///
        /// The optional TimeCodes tag shall contain an ordered array of time codes. All time codes shall be 8 bytes long and in binary format. The tag may contain from 1 to 10 time codes. When the tag contains more than one time code, the first one shall be the default time code. This specification does not prescribe how to use multiple time codes./// Each time code shall be as defined for the 8-byte time code structure in SMPTE 331M-2004, Section 8.3. See also SMPTE 12-1-2008 and SMPTE 309-1999.
        ///
        /// references:  \
        /// CinemaDNG specification 1.1.0 p10
        pub const TimeCodes: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TimeCodes",
            tag: 51043,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Time Codes of the Image",
            long_description: "The optional TimeCodes tag shall contain an ordered array of time codes. All time codes shall be 8 bytes long and in binary format. The tag may contain from 1 to 10 time codes. When the tag contains more than one time code, the first one shall be the default time code. This specification does not prescribe how to use multiple time codes.\nEach time code shall be as defined for the 8-byte time code structure in SMPTE 331M-2004, Section 8.3. See also SMPTE 12-1-2008 and SMPTE 309-1999.",
            references: "CinemaDNG specification 1.1.0 p10",
        }
    ;

    
        /// video frame rate in number of image frames per second
        ///
        /// The optional FrameRate tag shall specify the video frame rate in number of image frames per second, expressed as a signed rational number. The numerator shall be non-negative and the denominator shall be positive. This field value is identical to the sample rate field in SMPTE 377-1-2009.
        ///
        /// references:  \
        /// CinemaDNG specification 1.1.0 p11
        pub const FrameRate: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FrameRate",
            tag: 51044,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "video frame rate in number of image frames per second",
            long_description: "The optional FrameRate tag shall specify the video frame rate in number of image frames per second, expressed as a signed rational number. The numerator shall be non-negative and the denominator shall be positive. This field value is identical to the sample rate field in SMPTE 377-1-2009.",
            references: "CinemaDNG specification 1.1.0 p11",
        }
    ;

    
        /// T-stop of the actual lens
        ///
        /// The optional TStop tag shall specify the T-stop of the actual lens, expressed as an unsigned rational number. T-stop is also known as T-number or the photometric aperture of the lens. (F-number is the geometric aperture of the lens.) When the exact value is known, the T-stop shall be specified using a single number. Alternately, two numbers shall be used to indicate a T-stop range, in which case the first number shall be the minimum T-stop and the second number shall be the maximum T-stop.
        ///
        /// references:  \
        /// CinemaDNG specification 1.1.0 p11
        pub const TStop: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "TStop",
            tag: 51058,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "T-stop of the actual lens",
            long_description: "The optional TStop tag shall specify the T-stop of the actual lens, expressed as an unsigned rational number. T-stop is also known as T-number or the photometric aperture of the lens. (F-number is the geometric aperture of the lens.) When the exact value is known, the T-stop shall be specified using a single number. Alternately, two numbers shall be used to indicate a T-stop range, in which case the first number shall be the minimum T-stop and the second number shall be the maximum T-stop.",
            references: "CinemaDNG specification 1.1.0 p11",
        }
    ;

    
        /// name for a sequence of images
        ///
        /// The optional ReelName tag shall specify a name for a sequence of images, where each image in the sequence has a unique image identifier (including but not limited to file name, frame number, date time, time code).
        ///
        /// references:  \
        /// CinemaDNG specification 1.1.0 p11
        pub const ReelName: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ReelName",
            tag: 51081,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "name for a sequence of images",
            long_description: "The optional ReelName tag shall specify a name for a sequence of images, where each image in the sequence has a unique image identifier (including but not limited to file name, frame number, date time, time code).",
            references: "CinemaDNG specification 1.1.0 p11",
        }
    ;

    
        /// a text label for how the camera is used or assigned in this clip
        ///
        /// The optional CameraLabel tag shall specify a text label for how the camera is used or assigned in this clip. This tag is similar to CameraLabel in XMP.
        ///
        /// references:  \
        /// CinemaDNG specification 1.1.0 p12
        pub const CameraLabel: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CameraLabel",
            tag: 51105,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "a text label for how the camera is used or assigned in this clip",
            long_description: "The optional CameraLabel tag shall specify a text label for how the camera is used or assigned in this clip. This tag is similar to CameraLabel in XMP.",
            references: "CinemaDNG specification 1.1.0 p12",
        }
    ;

    
        /// name of the camera profile
        ///
        /// A UTF-8 encoded string containing the name of the camera profile. This tag is optional if there is only a single camera profile stored in the file but is required for all camera profiles if there is more than one camera profile stored in the file.
        ///
        /// references:  \
        /// DNG specification 1.4.0 p53
        pub const ProfileName: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ProfileName",
            tag: 50936,
            dtype: &[IfdValueType::Ascii, IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "name of the camera profile",
            long_description: "A UTF-8 encoded string containing the name of the camera profile. This tag is optional if there is only a single camera profile stored in the file but is required for all camera profiles if there is more than one camera profile stored in the file.",
            references: "DNG specification 1.4.0 p53",
        }
    ;

    
        /// tone curve that can be applied while processing the image as a starting point for user adjustments
        ///
        /// This tag contains a default tone curve that can be applied while processing the image as a starting point for user adjustments. The curve is specified as a list of 32-bit IEEE floating- point value pairs in linear gamma. Each sample has an input value in the range of 0.0 to 1.0, and an output value in the range of 0.0 to 1.0. The first sample is required to be (0.0, 0.0), and the last sample is required to be (1.0, 1.0). Interpolated the curve using a cubic spline.
        ///
        /// references:  \
        /// DNG specification 1.4.0 p56
        pub const ProfileToneCurve: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ProfileToneCurve",
            tag: 50940,
            dtype: &[IfdValueType::Float, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "tone curve that can be applied while processing the image as a starting point for user adjustments",
            long_description: "This tag contains a default tone curve that can be applied while processing the image as a starting point for user adjustments. The curve is specified as a list of 32-bit IEEE floating- point value pairs in linear gamma. Each sample has an input value in the range of 0.0 to 1.0, and an output value in the range of 0.0 to 1.0. The first sample is required to be (0.0, 0.0), and the last sample is required to be (1.0, 1.0). Interpolated the curve using a cubic spline.",
            references: "DNG specification 1.4.0 p56",
        }
    ;

    
        /// usage rules for the associated camera profile
        ///
        /// /// This tag contains information about the usage rules for the associated camera profile. The valid values and meanings are:/// • 0 = “allow copying”. The camera profile can be used to process, or be embedded in, any DNG file. It can be copied from DNG files to other DNG files, or copied from DNG files and stored on the user’s system for use in processing or embedding in any DNG file. The camera profile may not be used to process non-DNG files./// • 1 = “embed if used”. This value applies the same rules as “allow copying”, except it does not allow copying the camera profile from a DNG file for use in processing any image other than the image in which it is embedded, unless the profile is already stored on the user’s system./// • 2 = “embed never”. This value only applies to profiles stored on a user’s system but not already embedded in DNG files. These stored profiles can be used to process images but cannot be embedded in files. If a camera profile is already embedded in a DNG file, then this value has the same restrictions as “embed if used”./// • 3 = “no restrictions”. The camera profile creator has not placed any restrictions on the use of the camera profile.
        ///
        /// references:  \
        /// DNG specification 1.4.0 p57
        pub const ProfileEmbedPolicy: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ProfileEmbedPolicy",
            tag: 50941,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "allow copying"), (1, "embed if used"), (2, "embed never"), (3, "no restrictions"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "usage rules for the associated camera profile",
            long_description: "\nThis tag contains information about the usage rules for the associated camera profile. The valid values and meanings are:\n• 0 = “allow copying”. The camera profile can be used to process, or be embedded in, any DNG file. It can be copied from DNG files to other DNG files, or copied from DNG files and stored on the user’s system for use in processing or embedding in any DNG file. The camera profile may not be used to process non-DNG files.\n• 1 = “embed if used”. This value applies the same rules as “allow copying”, except it does not allow copying the camera profile from a DNG file for use in processing any image other than the image in which it is embedded, unless the profile is already stored on the user’s system.\n• 2 = “embed never”. This value only applies to profiles stored on a user’s system but not already embedded in DNG files. These stored profiles can be used to process images but cannot be embedded in files. If a camera profile is already embedded in a DNG file, then this value has the same restrictions as “embed if used”.\n• 3 = “no restrictions”. The camera profile creator has not placed any restrictions on the use of the camera profile.",
            references: "DNG specification 1.4.0 p57",
        }
    ;

    
        }
    
        /// Tags contained in the exif namespace
        #[allow(non_upper_case_globals)]
        pub mod exif {
            #[allow(unused_imports)]
            use super::{IfdFieldDescriptor, IfdValueType, IfdCount, IfdTypeInterpretation, IfdType};
            pub(crate) static ALL: [IfdFieldDescriptor; 58] = [ExposureTime, FNumber, ExposureProgram, SpectralSensitivity, ISOSpeedRatings, OECF, ExifVersion, DateTimeOriginal, DateTimeDigitized, ComponentsConfiguration, CompressedBitsPerPixel, ShutterSpeedValue, ApertureValue, BrightnessValue, ExposureBiasValue, MaxApertureValue, SubjectDistance, MeteringMode, LightSource, Flash, FocalLength, SubjectArea, MakerNote, UserComment, SubSecTime, SubSecTimeOriginal, SubSecTimeDigitized, FlashPixVersion, ColorSpace, PixelXDimension, PixelYDimension, RelatedSoundFile, InteroperabilityIFD, FlashEnergy, SpatialFrequencyResponse, FocalPlaneXResolution, FocalPlaneYResolution, FocalPlaneResolutionUnit, SubjectLocation, ExposureIndex, SensingMethod, FileSource, SceneType, CFAPattern, CustomRendered, ExposureMode, WhiteBalance, DigitalZoomRatio, FocalLengthIn35mmFilm, SceneCaptureType, GainControl, Contrast, Saturation, Sharpness, DeviceSettingDescription, SubjectDistanceRange, ImageUniqueID, Gamma, ];
            
        /// Exif picture exposure time
        ///
        /// The ExposureTime field contains how many seconds the frame was exposed.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const ExposureTime: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExposureTime",
            tag: 33434,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture exposure time",
            long_description: "The ExposureTime field contains how many seconds the frame was exposed.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture F number
        ///
        /// The FNumber field contains the F number of the picture.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const FNumber: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FNumber",
            tag: 33437,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture F number",
            long_description: "The FNumber field contains the F number of the picture.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture exposure program
        ///
        /// The ExposureProgram field describes the exposure setting program condition of the picture.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const ExposureProgram: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExposureProgram",
            tag: 34850,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Undefined"), (1, "Manual"), (2, "NormalProgram"), (3, "AperturePriority"), (4, "ShutterPriority"), (5, "CreativeProgram"), (6, "ActionProgram"), (7, "PortraitMode"), (8, "LandscapeMode"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture exposure program",
            long_description: "The ExposureProgram field describes the exposure setting program condition of the picture.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture spectral sensitivity
        ///
        /// The SpectralSensitivity field contains a description of the sensitivity of each channel of the image data according to ASTM standards.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const SpectralSensitivity: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SpectralSensitivity",
            tag: 34852,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif picture spectral sensitivity",
            long_description: "The SpectralSensitivity field contains a description of the sensitivity of each channel of the image data according to ASTM standards.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture ISO speed ratings
        ///
        /// The ISOSpeedRatings field contains the ISO speed or ISO latitude of the camera as specified by ISO 12232.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const ISOSpeedRatings: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ISOSpeedRatings",
            tag: 34855,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif picture ISO speed ratings",
            long_description: "The ISOSpeedRatings field contains the ISO speed or ISO latitude of the camera as specified by ISO 12232.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture optoelectronic conversion function
        ///
        /// The OECF field contains a specification of an opto-electronic conversion function as specified by ISO 14524.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const OECF: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "OECF",
            tag: 34856,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif picture optoelectronic conversion function",
            long_description: "The OECF field contains a specification of an opto-electronic conversion function as specified by ISO 14524.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif version
        ///
        /// The Exif version field contains the four ASCII characters "0210" to indicate Exif 2.1 conformance, or "0220" for Exif 2.2 conformance.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const ExifVersion: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExifVersion",
            tag: 36864,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(4),
            description: "Exif version",
            long_description: "The Exif version field contains the four ASCII characters \"0210\" to indicate Exif 2.1 conformance, or \"0220\" for Exif 2.2 conformance.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif origination date/time
        ///
        /// The DateTimeOriginal field contains 20 ASCII characters in the form "YYYY:MM:DD HH:MM:SS" indicating the date and time when the image data was sampled.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const DateTimeOriginal: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DateTimeOriginal",
            tag: 36867,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(20),
            description: "Exif origination date/time",
            long_description: "The DateTimeOriginal field contains 20 ASCII characters in the form \"YYYY:MM:DD HH:MM:SS\" indicating the date and time when the image data was sampled.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif digitization date/time
        ///
        /// The DateTimeDigitized field contains 20 ASCII characters in the form "YYYY:MM:DD HH:MM:SS" indicating the date  and time when the image data was digitized to file.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const DateTimeDigitized: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DateTimeDigitized",
            tag: 36868,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(20),
            description: "Exif digitization date/time",
            long_description: "The DateTimeDigitized field contains 20 ASCII characters in the form \"YYYY:MM:DD HH:MM:SS\" indicating the date  and time when the image data was digitized to file.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif component configuration
        ///
        /// The ComponentsConfiguration fields defines the order of components per the photometric interpretation, for describing other than default orders for RGB, CMYK, or YCbCr components.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const ComponentsConfiguration: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ComponentsConfiguration",
            tag: 37121,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(4),
            description: "Exif component configuration",
            long_description: "The ComponentsConfiguration fields defines the order of components per the photometric interpretation, for describing other than default orders for RGB, CMYK, or YCbCr components.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif compressed bits per pixel
        ///
        /// The CompressedBitsPerPixel field contains Exif data compression information.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const CompressedBitsPerPixel: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CompressedBitsPerPixel",
            tag: 37122,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif compressed bits per pixel",
            long_description: "The CompressedBitsPerPixel field contains Exif data compression information.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture shutter speed
        ///
        /// The ShutterSpeedValue field contains the APEX shutter speed setting.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const ShutterSpeedValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ShutterSpeedValue",
            tag: 37377,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture shutter speed",
            long_description: "The ShutterSpeedValue field contains the APEX shutter speed setting.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture lens aperture
        ///
        /// The ApertureValue field contains the APEX unit valued lens aperture.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const ApertureValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ApertureValue",
            tag: 37378,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture lens aperture",
            long_description: "The ApertureValue field contains the APEX unit valued lens aperture.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture brightness
        ///
        /// The BrightnessValue field contains the APEX unit valued brightness.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const BrightnessValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "BrightnessValue",
            tag: 37379,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture brightness",
            long_description: "The BrightnessValue field contains the APEX unit valued brightness.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture exposure bias
        ///
        /// The ExposureBiasValue field contains the APEX unit valued exposure bias.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const ExposureBiasValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExposureBiasValue",
            tag: 37380,
            dtype: &[IfdValueType::SRational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture exposure bias",
            long_description: "The ExposureBiasValue field contains the APEX unit valued exposure bias.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture lens maximum aperture
        ///
        /// The MaxApertureValue field contains the APEX unit valued minimum F number of the lens.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const MaxApertureValue: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "MaxApertureValue",
            tag: 37381,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture lens maximum aperture",
            long_description: "The MaxApertureValue field contains the APEX unit valued minimum F number of the lens.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture subject distance
        ///
        /// The SubjectDistance field contains the distance from the camera to the picture's subject, in meters.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const SubjectDistance: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubjectDistance",
            tag: 37382,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture subject distance",
            long_description: "The SubjectDistance field contains the distance from the camera to the picture's subject, in meters.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture metering mode
        ///
        /// The MeteringMode field describes the metering mode of the camera.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const MeteringMode: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "MeteringMode",
            tag: 37383,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Unidentified"), (1, "Average"), (2, "CenterWeightedAverage"), (3, "Spot"), (4, "MultiSpot"), (5, "Pattern"), (6, "Partial"), (255, "Other"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture metering mode",
            long_description: "The MeteringMode field describes the metering mode of the camera.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture light source
        ///
        /// The LightSource field describes the light source conditions of the picture.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a> ///  <a href="#EXIF22">EXIF22</a>
        pub const LightSource: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "LightSource",
            tag: 37384,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Unidentified"), (1, "Daylight"), (2, "Fluorescent"), (3, "Tungsten"), (4, "Flash"), (9, "FineWeather"), (10, "CloudyWeather"), (11, "Shady"), (12, "DaylightFluorescent"), (13, "DayWhiteFluorescent"), (14, "CoolWhiteFluorescent"), (15, "WhiteFluorescent"), (17, "StandardIlluminantA"), (18, "StandardIlluminantB"), (19, "StandardIlluminantC"), (20, "D55Illuminant"), (21, "D65Illuminant"), (22, "D75Illuminant"), (23, "D50Illuminant"), (24, "ISOStudioTungsten"), (255, "Other"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture light source",
            long_description: "The LightSource field describes the light source conditions of the picture.",
            references: "<a href=\"#EXIF21\">EXIF21</a> \n <a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture flash usage
        ///
        /// The Flash field describes the use of strobe flash with the picture.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a> ///  <a href="#EXIF22">EXIF22</a>
        pub const Flash: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Flash",
            tag: 37385,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture flash usage",
            long_description: "The Flash field describes the use of strobe flash with the picture.",
            references: "<a href=\"#EXIF21\">EXIF21</a> \n <a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture lens focal length
        ///
        /// The FocalLength field contains the focal length in millimeters of the lens.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const FocalLength: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FocalLength",
            tag: 37386,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture lens focal length",
            long_description: "The FocalLength field contains the focal length in millimeters of the lens.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture subject area
        ///
        /// The SubjectArea field contains a number of coordinates to describe either a point, sphere, or rectangle of the main subject area of the picture.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const SubjectArea: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubjectArea",
            tag: 37396,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif picture subject area",
            long_description: "The SubjectArea field contains a number of coordinates to describe either a point, sphere, or rectangle of the main subject area of the picture.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif maker note
        ///
        /// The MakerNote field contains information specific to the digital camera manufacturer.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const MakerNote: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "MakerNote",
            tag: 37500,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif maker note",
            long_description: "The MakerNote field contains information specific to the digital camera manufacturer.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif user comment
        ///
        /// The UserComment field contains a user comment.  The first eight bytes of the user comment data indicate the character encoding of the remaining comment data.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const UserComment: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "UserComment",
            tag: 37510,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif user comment",
            long_description: "The UserComment field contains a user comment.  The first eight bytes of the user comment data indicate the character encoding of the remaining comment data.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif subsecond time
        ///
        /// The SubSecTime field contains an ASCII string of as many significant digits as there are of the decimal fractions of a second associated with the DateTime field.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const SubSecTime: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubSecTime",
            tag: 37520,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif subsecond time",
            long_description: "The SubSecTime field contains an ASCII string of as many significant digits as there are of the decimal fractions of a second associated with the DateTime field.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif origination subsecond time
        ///
        /// The SubSecTime field contains an ASCII string of as many significant digits as there are of the decimal fractions of a second associated with the DateTimeOriginal field.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const SubSecTimeOriginal: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubSecTimeOriginal",
            tag: 37521,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif origination subsecond time",
            long_description: "The SubSecTime field contains an ASCII string of as many significant digits as there are of the decimal fractions of a second associated with the DateTimeOriginal field.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif digitization subsecond time
        ///
        /// The SubSecTime field contains an ASCII string of as many significant digits as there are of the decimal fractions of a second associated with the DateTimeDigitized field.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const SubSecTimeDigitized: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubSecTimeDigitized",
            tag: 37522,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif digitization subsecond time",
            long_description: "The SubSecTime field contains an ASCII string of as many significant digits as there are of the decimal fractions of a second associated with the DateTimeDigitized field.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif FlashPix version
        ///
        /// The FlashPixVersion field contains the conformance level to FlashPix interoperability, conformance to FlashPix 1.0 is indicated by the four ASCII characters "0100".
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const FlashPixVersion: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FlashPixVersion",
            tag: 40960,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(4),
            description: "Exif FlashPix version",
            long_description: "The FlashPixVersion field contains the conformance level to FlashPix interoperability, conformance to FlashPix 1.0 is indicated by the four ASCII characters \"0100\".",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif color space
        ///
        /// The ColorSpace field describes whether the Exif file uses the sRGB color space or is uncalibrated.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const ColorSpace: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ColorSpace",
            tag: 40961,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "sRGB (sRGB color space)"), (0xFFFF, "Unspecified (Unspecified color space)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif color space",
            long_description: "The ColorSpace field describes whether the Exif file uses the sRGB color space or is uncalibrated.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif unpadded columns
        ///
        /// The PixelXDimension field contains the number of imaged data sample columns in compressed data where there may be padding beyond the imaged width.
        ///
        /// references:  \
        /// See also<a href="#PixelYDimension">PixelYDimension</a>. <a href="#EXIF21">EXIF21</a>
        pub const PixelXDimension: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "PixelXDimension",
            tag: 40962,
            dtype: &[IfdValueType::Short, IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif unpadded columns",
            long_description: "The PixelXDimension field contains the number of imaged data sample columns in compressed data where there may be padding beyond the imaged width.",
            references: "See also<a href=\"#PixelYDimension\">PixelYDimension</a>. <a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif unpadded rows
        ///
        /// The PixelYDimension field contains the number of imaged data sample rows in compressed data.
        ///
        /// references:  \
        /// See also<a href="#PixelYDimension">PixelYDimension</a>. <a href="#EXIF21">EXIF21</a>
        pub const PixelYDimension: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "PixelYDimension",
            tag: 40963,
            dtype: &[IfdValueType::Short, IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif unpadded rows",
            long_description: "The PixelYDimension field contains the number of imaged data sample rows in compressed data.",
            references: "See also<a href=\"#PixelYDimension\">PixelYDimension</a>. <a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif related sound file
        ///
        /// The RelatedSoundFile field contains an ASCII filename of a sound file related to the image.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const RelatedSoundFile: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "RelatedSoundFile",
            tag: 40964,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(13),
            description: "Exif related sound file",
            long_description: "The RelatedSoundFile field contains an ASCII filename of a sound file related to the image.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif Interoperability sub-IFD pointer
        ///
        /// The InteroperabilityIFD field contains an offset to an IFD structure containing interoperability information.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const InteroperabilityIFD: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "InteroperabilityIFD",
            tag: 40965,
            dtype: &[IfdValueType::Long, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif Interoperability sub-IFD pointer",
            long_description: "The InteroperabilityIFD field contains an offset to an IFD structure containing interoperability information.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture flash energy
        ///
        /// The FlashEnergy field contains the power of the strobe flash in BCPS, beam candlepower seconds, units.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const FlashEnergy: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FlashEnergy",
            tag: 41483,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture flash energy",
            long_description: "The FlashEnergy field contains the power of the strobe flash in BCPS, beam candlepower seconds, units.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture spatial frequency response
        ///
        /// The SpatialFrequencyResponse field contains the device spatial frequency response table and values for the picture per ISO 12233.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const SpatialFrequencyResponse: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SpatialFrequencyResponse",
            tag: 41484,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif picture spatial frequency response",
            long_description: "The SpatialFrequencyResponse field contains the device spatial frequency response table and values for the picture per ISO 12233.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture focal plane column resolution
        ///
        /// The FocalPlaneXResolution field contains the focal plane column resolution in FocalPlaneResolutionUnits units.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const FocalPlaneXResolution: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FocalPlaneXResolution",
            tag: 41486,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture focal plane column resolution",
            long_description: "The FocalPlaneXResolution field contains the focal plane column resolution in FocalPlaneResolutionUnits units.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture focal plane row resolution
        ///
        /// The FocalPlaneXResolution field contains the focal plane row resolution in FocalPlaneResolutionUnit units.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const FocalPlaneYResolution: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FocalPlaneYResolution",
            tag: 41487,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture focal plane row resolution",
            long_description: "The FocalPlaneXResolution field contains the focal plane row resolution in FocalPlaneResolutionUnit units.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture focal plane resolution unit
        ///
        /// The FocalPlaneResolutionUnit field describes the focal plane resolution unit.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const FocalPlaneResolutionUnit: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FocalPlaneResolutionUnit",
            tag: 41488,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "Unitless (Units not specified)"), (2, "Inch (Units in inches)"), (3, "Centimeter (Units in centimeters)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture focal plane resolution unit",
            long_description: "The FocalPlaneResolutionUnit field describes the focal plane resolution unit.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture subject location
        ///
        /// The SubjectLocation field contains two coordinate values into the image of the pixel of the subject location in the picture.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const SubjectLocation: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubjectLocation",
            tag: 41492,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "Exif picture subject location",
            long_description: "The SubjectLocation field contains two coordinate values into the image of the pixel of the subject location in the picture.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture exposure index
        ///
        /// The ExposureIndex field contains the camera exposure index setting.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const ExposureIndex: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExposureIndex",
            tag: 41493,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture exposure index",
            long_description: "The ExposureIndex field contains the camera exposure index setting.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture sensing method
        ///
        /// The SensingMethod field describes the sensors that capture the image of the picture.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const SensingMethod: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SensingMethod",
            tag: 41495,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "Undefined"), (2, "OneChipColorArea"), (3, "TwoChipColorArea"), (4, "ThreeChipColorArea"), (5, "ColorSequentialArea"), (7, "TriLinear"), (8, "ColorSequentialLinear"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture sensing method",
            long_description: "The SensingMethod field describes the sensors that capture the image of the picture.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture data file source
        ///
        /// The FileSource field describes the source of the image data of this picture.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a> ///  <a href="#EXIF221">EXIF221</a>
        pub const FileSource: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FileSource",
            tag: 41728,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "OtherSource"), (1, "TransparentScannerSource"), (2, "ReflexScannerSource"), (3, "DSCSource"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture data file source",
            long_description: "The FileSource field describes the source of the image data of this picture.",
            references: "<a href=\"#EXIF21\">EXIF21</a> \n <a href=\"#EXIF221\">EXIF221</a>",
        }
    ;

    
        /// Exif picture scene type
        ///
        /// The SceneType field describes the picture-taking conditions of the Exif picture.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const SceneType: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SceneType",
            tag: 41729,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(1, "DirectlyPhotographed"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture scene type",
            long_description: "The SceneType field describes the picture-taking conditions of the Exif picture.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture color filter array pattern
        ///
        /// The CFAPattern field contains a description of the color filter array geometric pattern for interleaving of sampling channels.
        ///
        /// references:  \
        /// See also<a href="#SensingMethod">SensingMethod</a>. <a href="#EXIF21">EXIF21</a>
        pub const CFAPattern: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CFAPattern",
            tag: 41730,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::CfaPattern,
            count: IfdCount::N,
            description: "Exif picture color filter array pattern",
            long_description: "The CFAPattern field contains a description of the color filter array geometric pattern for interleaving of sampling channels.",
            references: "See also<a href=\"#SensingMethod\">SensingMethod</a>. <a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// Exif picture custom rendering
        ///
        /// The CustomRendering field describes whether the picture data has been custom rendered for the output.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const CustomRendered: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "CustomRendered",
            tag: 41985,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "NormalProcess"), (1, "CustomProcess"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture custom rendering",
            long_description: "The CustomRendering field describes whether the picture data has been custom rendered for the output.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture exposure mode
        ///
        /// The ExposureMode field describes the exposure mode (auto exposure, manual exposure, auto bracketing) of the picture or picture sequence.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const ExposureMode: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ExposureMode",
            tag: 41986,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "AutoExposure"), (1, "ManualExposure"), (2, "AutoBracket"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture exposure mode",
            long_description: "The ExposureMode field describes the exposure mode (auto exposure, manual exposure, auto bracketing) of the picture or picture sequence.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture white balance mode
        ///
        /// The WhiteBalance field describes the white balance mode of the picture, auto or manual.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const WhiteBalance: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "WhiteBalance",
            tag: 41987,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "AutoWhiteBalance"), (1, "ManualWhiteBalance"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture white balance mode",
            long_description: "The WhiteBalance field describes the white balance mode of the picture, auto or manual.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture digital zoom ratio
        ///
        /// The DigitalZoomRatio field contains the digital zoom ratio of the picture.  If the numerator of the field value is zero then digital zoom was not used.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const DigitalZoomRatio: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DigitalZoomRatio",
            tag: 41988,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture digital zoom ratio",
            long_description: "The DigitalZoomRatio field contains the digital zoom ratio of the picture.  If the numerator of the field value is zero then digital zoom was not used.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture 35mm lens focal length
        ///
        /// The FocalLengthIn35mmFilm field contains the equivalent of the lens focal length to a 35mm film camera lens.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const FocalLengthIn35mmFilm: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "FocalLengthIn35mmFilm",
            tag: 41989,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture 35mm lens focal length",
            long_description: "The FocalLengthIn35mmFilm field contains the equivalent of the lens focal length to a 35mm film camera lens.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture scene capture type
        ///
        /// The SceneCaptureType field describes the scene type in terms of standard, landscape, portrait, night scene, etcetera.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const SceneCaptureType: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SceneCaptureType",
            tag: 41990,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Standard"), (1, "Portrait"), (2, "Landscape"), (3, "Night"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture scene capture type",
            long_description: "The SceneCaptureType field describes the scene type in terms of standard, landscape, portrait, night scene, etcetera.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture gain control
        ///
        /// The GainControl field describes the image gain adjustment of the picture.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const GainControl: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GainControl",
            tag: 41991,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "NoGainControl"), (1, "LowGainUp"), (2, "HighGainUp"), (3, "LowGainDown"), (4, "HighGainDown"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture gain control",
            long_description: "The GainControl field describes the image gain adjustment of the picture.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture contrast adjustment
        ///
        /// The Contrast field describes the contrast adjustment on the picture.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const Contrast: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Contrast",
            tag: 41992,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Normal"), (1, "Soft"), (2, "Hard"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture contrast adjustment",
            long_description: "The Contrast field describes the contrast adjustment on the picture.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture saturation processing
        ///
        /// The Saturation field describes the saturation processing on the picture.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const Saturation: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Saturation",
            tag: 41993,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Normal"), (1, "LowSaturation"), (2, "HighSaturation"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture saturation processing",
            long_description: "The Saturation field describes the saturation processing on the picture.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture sharpness processing
        ///
        /// The Sharpness field describes the sharpness processing on the picture.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const Sharpness: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Sharpness",
            tag: 41994,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "Normal"), (1, "Soft"), (2, "Hard"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture sharpness processing",
            long_description: "The Sharpness field describes the sharpness processing on the picture.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture device conditions
        ///
        /// The DeviceSettingDesciption field contains a list of device setting descriptions of the picture-taking condition of the camera.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a> ///  <a href="#EXIF221">EXIF221</a>
        pub const DeviceSettingDescription: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "DeviceSettingDescription",
            tag: 41995,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "Exif picture device conditions",
            long_description: "The DeviceSettingDesciption field contains a list of device setting descriptions of the picture-taking condition of the camera.",
            references: "<a href=\"#EXIF22\">EXIF22</a> \n <a href=\"#EXIF221\">EXIF221</a>",
        }
    ;

    
        /// Exif picture subject distance range
        ///
        /// The SubjectDistanceRange field describes the distance range of the picture subject.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const SubjectDistanceRange: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "SubjectDistanceRange",
            tag: 41996,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif picture subject distance range",
            long_description: "The SubjectDistanceRange field describes the distance range of the picture subject.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif picture unique identifier
        ///
        /// The ImageUniqueID field contains a null terminated ASCII string of hexadecimal representation of an 128 bit unique identifier of the picture.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const ImageUniqueID: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "ImageUniqueID",
            tag: 42016,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(33),
            description: "Exif picture unique identifier",
            long_description: "The ImageUniqueID field contains a null terminated ASCII string of hexadecimal representation of an 128 bit unique identifier of the picture.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// Exif Gamma
        ///
        /// The Gamma field contains an value between 0 and 1 indiciating the normalized gamma coefficient.
        ///
        /// references:  \
        /// <a href="#EXIF221">EXIF221</a>
        pub const Gamma: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "Gamma",
            tag: 42240,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "Exif Gamma",
            long_description: "The Gamma field contains an value between 0 and 1 indiciating the normalized gamma coefficient.",
            references: "<a href=\"#EXIF221\">EXIF221</a>",
        }
    ;

    
        }
    
        /// Tags contained in the gps_info namespace
        #[allow(non_upper_case_globals)]
        pub mod gps_info {
            #[allow(unused_imports)]
            use super::{IfdFieldDescriptor, IfdValueType, IfdCount, IfdTypeInterpretation, IfdType};
            pub(crate) static ALL: [IfdFieldDescriptor; 31] = [GPSVersionID, GPSLatitudeRef, GPSLatitude, GPSLongitudeRef, GPSLongitude, GPSAltitudeRef, GPSAltitude, GPSTimeStamp, GPSSatellites, GPSStatus, GPSMeasureMode, GPSDOP, GPSSpeedRef, GPSSpeed, GPSTrackRef, GPSTrack, GPSImgDirectionRef, GPSImgDirection, GPSMapDatum, GPSDestLatitudeRef, GPSDestLatitude, GPSDestLongitudeRef, GPSDestLongitude, GPSDestBearingRef, GPSDestBearing, GPSDestDistanceRef, GPSDestDistance, GPSProcessingMethod, GPSAreaInformation, GPSDateStamp, GPSDifferential, ];
            
        /// GPSInfo Version of GPSInfoIFD
        ///
        /// The GPSVersionID fields contains four bytes that represent a version number x.x.x.x.  The values are the numeric values of the bytes.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSVersionID: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSVersionID",
            tag: 0,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(4),
            description: "GPSInfo Version of GPSInfoIFD",
            long_description: "The GPSVersionID fields contains four bytes that represent a version number x.x.x.x.  The values are the numeric values of the bytes.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo north or south latitude
        ///
        /// The GPSLatitudeRef field contains an ASCII null-terminated string of "N" for north or "S" for south.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSLatitudeRef: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSLatitudeRef",
            tag: 1,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo north or south latitude",
            long_description: "The GPSLatitudeRef field contains an ASCII null-terminated string of \"N\" for north or \"S\" for south.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo latitude
        ///
        /// The GPSLatitude field contains three values, one each for degrees, minutes, and seconds.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSLatitude: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSLatitude",
            tag: 2,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(3),
            description: "GPSInfo latitude",
            long_description: "The GPSLatitude field contains three values, one each for degrees, minutes, and seconds.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo east or west longitude
        ///
        /// The GPSLongitudeRef field contains an ASCII null-terminated string of "E" for east or "W" for west.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSLongitudeRef: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSLongitudeRef",
            tag: 3,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo east or west longitude",
            long_description: "The GPSLongitudeRef field contains an ASCII null-terminated string of \"E\" for east or \"W\" for west.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo longitude
        ///
        /// The GPSLongitude field contains three values, one each for degrees, minutes, and seconds.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSLongitude: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSLongitude",
            tag: 4,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(3),
            description: "GPSInfo longitude",
            long_description: "The GPSLongitude field contains three values, one each for degrees, minutes, and seconds.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo altitude reference
        ///
        /// The GPSAltitudeRef contains a value that describes the altitude reference.  The default and only defined value is SeaLevel == 0.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSAltitudeRef: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSAltitudeRef",
            tag: 5,
            dtype: &[IfdValueType::Byte, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "GPSInfo altitude reference",
            long_description: "The GPSAltitudeRef contains a value that describes the altitude reference.  The default and only defined value is SeaLevel == 0.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo altitude
        ///
        /// The GPSAltitude field contains the value in meters of altitude from the altitude reference.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSAltitude: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSAltitude",
            tag: 6,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "GPSInfo altitude",
            long_description: "The GPSAltitude field contains the value in meters of altitude from the altitude reference.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo time stamp
        ///
        /// The GPSTimeStamp field contains three values representing hours, minutes, and seconds of a UTC timestamp.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSTimeStamp: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSTimeStamp",
            tag: 7,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(3),
            description: "GPSInfo time stamp",
            long_description: "The GPSTimeStamp field contains three values representing hours, minutes, and seconds of a UTC timestamp.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo satellites
        ///
        /// The GPSSatellites field contains information about the satellites that provided the GPS information.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSSatellites: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSSatellites",
            tag: 8,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "GPSInfo satellites",
            long_description: "The GPSSatellites field contains information about the satellites that provided the GPS information.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo receiver status
        ///
        /// The GPSStatus field contains an ASCII null-terminated string of "A" for acquisition or "V" for received.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSStatus: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSStatus",
            tag: 9,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo receiver status",
            long_description: "The GPSStatus field contains an ASCII null-terminated string of \"A\" for acquisition or \"V\" for received.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo measure mode
        ///
        /// The GPSMeasureMode field contains an ASCII null-terminated string of "2" for 2-dimensional measurement or "3" for 3-dimensional measurement.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSMeasureMode: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSMeasureMode",
            tag: 10,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo measure mode",
            long_description: "The GPSMeasureMode field contains an ASCII null-terminated string of \"2\" for 2-dimensional measurement or \"3\" for 3-dimensional measurement.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo data degree of precision
        ///
        /// The GPSDOP field contains an HDOP for 2-dimensional precision or a PDOP for 3-dimensional precisio.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSDOP: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDOP",
            tag: 11,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "GPSInfo data degree of precision",
            long_description: "The GPSDOP field contains an HDOP for 2-dimensional precision or a PDOP for 3-dimensional precisio.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo speed reference
        ///
        /// The GPSSpeedRef field contains an ASCII null-terminated string of "K" for kilometers per hour, "M" for miles per hour, or "N" for knots.  The default is "K".
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSSpeedRef: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSSpeedRef",
            tag: 12,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo speed reference",
            long_description: "The GPSSpeedRef field contains an ASCII null-terminated string of \"K\" for kilometers per hour, \"M\" for miles per hour, or \"N\" for knots.  The default is \"K\".",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo receiver speed
        ///
        /// The GPSSpeed field contains the speed of the receiver movement.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSSpeed: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSSpeed",
            tag: 13,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "GPSInfo receiver speed",
            long_description: "The GPSSpeed field contains the speed of the receiver movement.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo tracking reference
        ///
        /// The GPSTrackRef field contains an ASCII null-terminated string of "T" for true direction or "M" for magnetic direction.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSTrackRef: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSTrackRef",
            tag: 14,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo tracking reference",
            long_description: "The GPSTrackRef field contains an ASCII null-terminated string of \"T\" for true direction or \"M\" for magnetic direction.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo tracking direction
        ///
        /// The GPSTrack field contains a value between 0 and 359.99 indicating the direction of GPS receiver movement.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSTrack: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSTrack",
            tag: 15,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "GPSInfo tracking direction",
            long_description: "The GPSTrack field contains a value between 0 and 359.99 indicating the direction of GPS receiver movement.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo image capture direction reference
        ///
        /// The GPSImgDirectionRef field contains an ASCII null-terminated string of "T" for true direction or "M" for magnetic direction.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSImgDirectionRef: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSImgDirectionRef",
            tag: 16,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo image capture direction reference",
            long_description: "The GPSImgDirectionRef field contains an ASCII null-terminated string of \"T\" for true direction or \"M\" for magnetic direction.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPS image capture direction
        ///
        /// The GPSImgDirection field contains a value between 0 and 359.99 indicating the direction of GPS receiver movement.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSImgDirection: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSImgDirection",
            tag: 17,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "GPS image capture direction",
            long_description: "The GPSImgDirection field contains a value between 0 and 359.99 indicating the direction of GPS receiver movement.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo geodetic survey
        ///
        /// The GPSMapDatum field contains the geodetic survey data used by the GPS receiver.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSMapDatum: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSMapDatum",
            tag: 18,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "GPSInfo geodetic survey",
            long_description: "The GPSMapDatum field contains the geodetic survey data used by the GPS receiver.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo destination north or south latitude
        ///
        /// The GPSDestLatitudeRef field contains an ASCII null-terminated string of "N" for north or "S" for south.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSDestLatitudeRef: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDestLatitudeRef",
            tag: 19,
            dtype: &[],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo destination north or south latitude",
            long_description: "The GPSDestLatitudeRef field contains an ASCII null-terminated string of \"N\" for north or \"S\" for south.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo destination latitude
        ///
        /// The GPSDestLatitude field contains three values, one each for degrees, minutes, and seconds.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSDestLatitude: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDestLatitude",
            tag: 20,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(3),
            description: "GPSInfo destination latitude",
            long_description: "The GPSDestLatitude field contains three values, one each for degrees, minutes, and seconds.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo destination east or west longitude
        ///
        /// The GPSDestLongitudeRef field contains an ASCII null-terminated string of "E" for east or "W" for west.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSDestLongitudeRef: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDestLongitudeRef",
            tag: 21,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo destination east or west longitude",
            long_description: "The GPSDestLongitudeRef field contains an ASCII null-terminated string of \"E\" for east or \"W\" for west.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo destination longitude
        ///
        /// The GPSDestLongitude field contains three values, one each for degrees, minutes, and seconds.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSDestLongitude: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDestLongitude",
            tag: 22,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(3),
            description: "GPSInfo destination longitude",
            long_description: "The GPSDestLongitude field contains three values, one each for degrees, minutes, and seconds.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo destination bearing reference
        ///
        /// The GPSDestBearingRef field contains an ASCII null-terminated string of "T" for true direction or "M" for magnetic direction.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSDestBearingRef: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDestBearingRef",
            tag: 23,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo destination bearing reference",
            long_description: "The GPSDestBearingRef field contains an ASCII null-terminated string of \"T\" for true direction or \"M\" for magnetic direction.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo destination bearing
        ///
        /// The GPSDestBearing field contains a value between 0 and 359.99 indicating the direction of the destination bearing.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSDestBearing: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDestBearing",
            tag: 24,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "GPSInfo destination bearing",
            long_description: "The GPSDestBearing field contains a value between 0 and 359.99 indicating the direction of the destination bearing.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo destination distance reference
        ///
        /// The GPSDestDistanceRef field contains an ASCII null-terminated string of "K" for kilometers, "M" for miles, or "N" for knots.  The default is "K".
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSDestDistanceRef: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDestDistanceRef",
            tag: 25,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(2),
            description: "GPSInfo destination distance reference",
            long_description: "The GPSDestDistanceRef field contains an ASCII null-terminated string of \"K\" for kilometers, \"M\" for miles, or \"N\" for knots.  The default is \"K\".",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo destination distance
        ///
        /// The GPSDestDistance field contains the distance to the destination point.
        ///
        /// references:  \
        /// <a href="#EXIF21">EXIF21</a>
        pub const GPSDestDistance: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDestDistance",
            tag: 26,
            dtype: &[IfdValueType::Rational, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(1),
            description: "GPSInfo destination distance",
            long_description: "The GPSDestDistance field contains the distance to the destination point.",
            references: "<a href=\"#EXIF21\">EXIF21</a>",
        }
    ;

    
        /// GPSInfo processing method
        ///
        /// The GPSProcessingMethod field contains a character string recording the name of the GPS area.  The first byte is a code reflecting the character set, the remaining bytes represent the string content, there is no necessary null termination.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const GPSProcessingMethod: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSProcessingMethod",
            tag: 27,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "GPSInfo processing method",
            long_description: "The GPSProcessingMethod field contains a character string recording the name of the GPS area.  The first byte is a code reflecting the character set, the remaining bytes represent the string content, there is no necessary null termination.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// GPSInfo area information
        ///
        /// The GPSAreaInformation field contains a character string recording the name of the GPS area.  The first byte is a code reflecting the character set, the remaining bytes represent the string content, there is no necessary null termination.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const GPSAreaInformation: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSAreaInformation",
            tag: 28,
            dtype: &[IfdValueType::Undefined, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::N,
            description: "GPSInfo area information",
            long_description: "The GPSAreaInformation field contains a character string recording the name of the GPS area.  The first byte is a code reflecting the character set, the remaining bytes represent the string content, there is no necessary null termination.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// GPSInfo date stamp
        ///
        /// The GPSDateStamp field contains an ASCII string representing a UTC time in the form "YYYY:MM:DD".
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const GPSDateStamp: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDateStamp",
            tag: 29,
            dtype: &[IfdValueType::Ascii, ],
            interpretation: IfdTypeInterpretation::Default,
            count: IfdCount::ConcreteValue(11),
            description: "GPSInfo date stamp",
            long_description: "The GPSDateStamp field contains an ASCII string representing a UTC time in the form \"YYYY:MM:DD\".",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        /// GPSInfo differential correction
        ///
        /// The GPSDifferential field describes whether differential correction is applied to the GPS receiver.
        ///
        /// references:  \
        /// <a href="#EXIF22">EXIF22</a>
        pub const GPSDifferential: IfdFieldDescriptor = 
        IfdFieldDescriptor {
            name: "GPSDifferential",
            tag: 30,
            dtype: &[IfdValueType::Short, ],
            interpretation: IfdTypeInterpretation::Enumerated { values: &[(0, "NoDifferentialCorrection (Measurement without differential correction)"), (1, "WithDifferentialCorrection (Differential correction applied)"), ] },
            count: IfdCount::ConcreteValue(1),
            description: "GPSInfo differential correction",
            long_description: "The GPSDifferential field describes whether differential correction is applied to the GPS receiver.",
            references: "<a href=\"#EXIF22\">EXIF22</a>",
        }
    ;

    
        }
    