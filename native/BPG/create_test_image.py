#!/usr/bin/env python3
"""Create a simple test image for BPG encoder testing"""

try:
    from PIL import Image, ImageDraw, ImageFont
    import sys
    
    # Create a 800x600 test image
    img = Image.new('RGB', (800, 600), color=(70, 130, 180))
    
    # Draw some shapes
    draw = ImageDraw.Draw(img)
    
    # Draw rectangles
    draw.rectangle([50, 50, 250, 250], fill=(255, 100, 100), outline=(255, 255, 255), width=3)
    draw.rectangle([300, 50, 500, 250], fill=(100, 255, 100), outline=(255, 255, 255), width=3)
    draw.rectangle([550, 50, 750, 250], fill=(100, 100, 255), outline=(255, 255, 255), width=3)
    
    # Draw circles
    draw.ellipse([50, 300, 250, 500], fill=(255, 255, 100), outline=(255, 255, 255), width=3)
    draw.ellipse([300, 300, 500, 500], fill=(255, 100, 255), outline=(255, 255, 255), width=3)
    draw.ellipse([550, 300, 750, 500], fill=(100, 255, 255), outline=(255, 255, 255), width=3)
    
    # Add text
    try:
        font = ImageFont.truetype("arial.ttf", 40)
    except:
        font = ImageFont.load_default()
    
    draw.text((250, 550), "BPG Test Image", fill=(255, 255, 255), font=font)
    
    # Save as PNG and JPG
    img.save('test_input.png')
    img.save('test_input.jpg', quality=95)
    
    print("Test images created successfully!")
    print("  - test_input.png")
    print("  - test_input.jpg")
    
except ImportError:
    print("PIL/Pillow not available. Creating a simple PPM file instead...")
    
    # Create a simple PPM file (no dependencies)
    width, height = 800, 600
    with open('test_input.ppm', 'w') as f:
        f.write(f'P3\n{width} {height}\n255\n')
        for y in range(height):
            for x in range(width):
                # Create a gradient
                r = int(255 * x / width)
                g = int(255 * y / height)
                b = 128
                f.write(f'{r} {g} {b} ')
            f.write('\n')
    
    print("Created test_input.ppm (PPM format)")
    print("Note: bpgenc may not support PPM. Please convert to PNG or JPG first.")
