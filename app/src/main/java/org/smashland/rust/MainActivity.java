package org.smashland.rust;

import android.util.Log;
import org.libsdl.app.SDLActivity;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/** SDL hosts the shared Rust game; packaged assets are unpacked into private storage. */
public final class MainActivity extends SDLActivity {
    @Override protected String[] getLibraries() { return new String[] {"SDL3", "smash_filament", "main"}; }

    @Override protected void main() {
        try { prepareAssets(); }
        catch (IOException error) {
            Log.e("SmashLand", "Cannot prepare game assets", error);
            runOnUiThread(() -> new android.app.AlertDialog.Builder(this)
                .setTitle("Smash Land").setMessage("Cannot prepare game files: " + error.getMessage())
                .setPositiveButton("Close", (dialog, which) -> finish()).show());
            return;
        }
        super.main();
    }

    private String readText(InputStream input) throws IOException {
        try (InputStream in = input; ByteArrayOutputStream out = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[4096]; int count;
            while ((count = in.read(buffer)) != -1) out.write(buffer, 0, count);
            return new String(out.toByteArray(), StandardCharsets.UTF_8).trim();
        }
    }

    private void prepareAssets() throws IOException {
        File directory = getFilesDir();
        String version = readText(getAssets().open("game-assets.sha256"));
        File marker = new File(directory, ".assets-version");
        if (marker.isFile() && readText(new FileInputStream(marker)).equals(version)) return;
        String prefix = directory.getCanonicalPath() + File.separator;
        byte[] buffer = new byte[65536];
        try (ZipInputStream zip = new ZipInputStream(new BufferedInputStream(getAssets().open("game-assets.zip")))) {
            ZipEntry entry;
            while ((entry = zip.getNextEntry()) != null) {
                File target = new File(directory, entry.getName());
                if (!target.getCanonicalPath().startsWith(prefix)) throw new IOException("Invalid asset path");
                if (entry.isDirectory()) { if (!target.isDirectory() && !target.mkdirs()) throw new IOException("Cannot create " + target); continue; }
                File parent = target.getParentFile();
                if (!parent.isDirectory() && !parent.mkdirs()) throw new IOException("Cannot create " + parent);
                File temporary = new File(target.getPath() + ".part");
                try (OutputStream out = new BufferedOutputStream(new FileOutputStream(temporary))) {
                    int count; while ((count = zip.read(buffer)) != -1) out.write(buffer, 0, count);
                }
                if (!temporary.renameTo(target)) throw new IOException("Cannot replace " + target);
            }
        }
        try (OutputStream out = new FileOutputStream(marker)) { out.write(version.getBytes(StandardCharsets.UTF_8)); }
    }
}
