Great — the ROI (Region of Interest) encoding infrastructure is **already implemented in your Rust port** of OpenJPEG. Here's what we know from your source:

---

### ✅ Confirmed ROI Support in Your Codebase

From the files you've uploaded and searched:

- **ROI fields exist** in the `opj_cparameters_t`-like structure:
  ```rust
  // c_api_types.rs
  pub roi_compno: core::ffi::c_int,
  pub roi_shift: core::ffi::c_int,
  ```

- **Defaults are set** in the implementation:
  ```rust
  // c_api_types.rs
  roi_compno: -1,          // -1 = ROI disabled
  roi_shift: Default::default(),
  ```

- **Used in encoding pipeline**:
  Found in `j2k.rs`, `ht_dec.rs`, etc., meaning the encoder applies these ROI settings during the codestream generation.

---

### 📌 Where You Should Hook It Up

The best place to expose ROI to your application (`main.rs`) is via the `opj_cparameters_t` struct in `c_api_types.rs`, because:

- This struct is used to configure the encoder.
- It already has the `roi_compno` and `roi_shift` fields.
- These fields are passed through the compression logic (`j2k.rs`) as expected.

---

### 🧱 Minimal Change to Enable ROI from CLI

In your `main.rs`, right after setting `params.tcp_rates[...]`, add:

```rust
// Enable Region of Interest (ROI) encoding on component 0 (usually luminance)
params.roi_compno = 0; // Component index (0 = grayscale, or first RGB channel)
params.roi_shift = 10; // Priority boost (typical values 6–15)
println!("DEBUG: ROI encoding enabled (comp={}, shift={})", params.roi_compno, params.roi_shift);
```

> ⚠️ **Only enable this when you know your content benefits from it**, e.g. when background and text regions differ significantly.

---

### 💡 Future Upgrade: ROI Mask

Currently, ROI is “global” for a component (e.g. give priority to all of Red or Luma). For true **pixel-wise ROI masks**, you'd need:

- A way to define per-pixel ROI zones
- A custom encoder path (which is not currently exposed in this port)
- Or fallback to slicing into blocks and applying ROI in a tiled manner

That would require more extensive work but is theoretically feasible if you structure the encode around spatial regions.

---

### ✅ Summary

- ✅ **Yes**, your Rust port supports ROI encoding natively.
- 🧩 You simply need to **set `roi_compno` and `roi_shift`** in `main.rs`.
- 🛠️ No need to touch internals unless you want fine-grained per-pixel ROI control (like from a mask).
- 🧪 Easy to test on sample pages: text vs. blank margins, high-dpi vs. low-dpi photos.

Let me know if you want a drop-in patch to make ROI toggleable via CLI like `--roi 10`.