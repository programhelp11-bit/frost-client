package dev.frost.core;

import com.google.gson.Gson;
import com.google.gson.JsonObject;
import com.google.gson.reflect.TypeToken;

import java.util.HashMap;
import java.util.Map;

/** Applies launcher -> game messages (ToGame in src/ipc.rs) to {@link FrostState}. */
public final class Dispatcher {
    private static final Gson GSON = new Gson();
    private static final java.lang.reflect.Type PLAYERS =
        new TypeToken<HashMap<String, Models.CosmeticEntry>>() {}.getType();

    private Dispatcher() {}

    public static void handle(FrostState state, JsonObject msg) {
        if (!msg.has("type")) return;
        switch (msg.get("type").getAsString()) {
            case "settings" -> {
                if (msg.has("qol")) state.qol = GSON.fromJson(msg.get("qol"), Models.Qol.class);
                if (msg.has("hud")) state.hud = GSON.fromJson(msg.get("hud"), Models.HudLayout.class);
            }
            case "cosmetics" -> {
                Map<String, Models.CosmeticEntry> players = GSON.fromJson(msg.get("players"), PLAYERS);
                state.cosmetics = players == null ? Map.of() : players;
            }
            case "spawn_bot" -> state.spawnQueue.add(GSON.fromJson(msg.get("bot"), Models.BotConfig.class));
            case "clear_bots" -> state.clearBots.set(true);
            case "knockback_trainer" -> {
                state.knockbackTrainer = msg.get("enabled").getAsBoolean();
                if (msg.has("profile")) state.kbProfile = GSON.fromJson(msg.get("profile"), Models.KbProfile.class);
            }
            case "bot_command" -> state.botCommands.put(
                msg.get("id").getAsInt(), GSON.fromJson(msg.get("command"), Models.BotCommand.class));
            default -> { }
        }
    }

    public static Gson gson() {
        return GSON;
    }
}
