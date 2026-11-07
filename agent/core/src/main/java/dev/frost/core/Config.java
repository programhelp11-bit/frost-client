package dev.frost.core;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import java.io.File;
import java.io.FileReader;
import java.io.FileWriter;
import java.io.Reader;
import java.io.Writer;

/** Stores the player's settings and HUD layout in a JSON file next to the other mod configs. */
public final class Config {
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    private Config() {}

    public static Gson gson() {
        return GSON;
    }

    static final class Data {
        Models.Qol qol;
        Models.HudLayout hud;
    }

    public static void load(File file, FrostState st) {
        if (file == null || !file.isFile()) return;
        try (Reader r = new FileReader(file)) {
            Data d = GSON.fromJson(r, Data.class);
            if (d == null) return;
            if (d.qol != null) st.qol = d.qol;
            if (d.hud != null && d.hud.widgets != null) st.hud = d.hud.merged();
        } catch (Exception ignored) {
            // a broken file simply means defaults
        }
    }

    public static void save(File file, FrostState st) {
        if (file == null) return;
        try {
            File dir = file.getParentFile();
            if (dir != null) dir.mkdirs();
            Data d = new Data();
            d.qol = st.qol;
            d.hud = st.hud;
            try (Writer w = new FileWriter(file)) {
                GSON.toJson(d, w);
            }
        } catch (Exception ignored) {
            // read-only config folder: settings just are not remembered
        }
    }
}
