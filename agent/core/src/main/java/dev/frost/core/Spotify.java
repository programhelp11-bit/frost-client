package dev.frost.core;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.awt.image.BufferedImage;
import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.InetSocketAddress;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.Base64;
import java.util.concurrent.TimeUnit;
import javax.imageio.ImageIO;

/**
 * Spotify control and "now playing" for the HUD.
 * <ul>
 *   <li>Web API (cover art, progress, works on any OS): needs a free Spotify developer app id
 *       (Features.spotify_client_id) and a one-time browser login.</li>
 *   <li>Without a login it falls back to the local player: playerctl on Linux, window title and media keys on Windows.</li>
 * </ul>
 */
public final class Spotify {
    public static final class Track {
        public String title = "", artist = "", album = "";
        public long progressMs, durationMs;
        public boolean playing;
        /** 16x16 cover art as ARGB, or null. */
        public int[] art;
        public String source = "";
    }

    public static volatile Track track = new Track();

    private static final String REDIRECT = "http://127.0.0.1:8899/callback";
    private static volatile boolean started;
    private static String access = "";
    private static long accessUntil;
    private static String artUrl = "";
    private static int[] artPixels;
    private static String verifier = "";
    private static com.sun.net.httpserver.HttpServer loginServer;

    private Spotify() {}

    private static boolean windows() {
        return System.getProperty("os.name", "").toLowerCase().contains("win");
    }

    // ------------------------------------------------------------------ polling

    public static void start(final FrostState st, final Runnable save) {
        if (started) return;
        started = true;
        Thread t = new Thread(new Runnable() {
            public void run() {
                while (true) {
                    try {
                        Models.Features f = st.fx;
                        if (f.spotify_enabled) {
                            Track tr = null;
                            if (!f.spotify_refresh.isEmpty() && !f.spotify_client_id.isEmpty()) tr = web(st);
                            if (tr == null) tr = local();
                            if (tr != null) track = tr;
                        }
                    } catch (Throwable ignored) {
                        // keep polling
                    }
                    try {
                        Thread.sleep(1500L);
                    } catch (InterruptedException e) {
                        return;
                    }
                }
            }
        }, "frost-spotify");
        t.setDaemon(true);
        t.start();
    }

    // ------------------------------------------------------------------ web api

    private static String http(String method, String url, String body, String bearer, String contentType) throws Exception {
        HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
        c.setRequestMethod(method);
        c.setConnectTimeout(6000);
        c.setReadTimeout(8000);
        if (bearer != null) c.setRequestProperty("Authorization", "Bearer " + bearer);
        if (body != null) {
            c.setDoOutput(true);
            c.setRequestProperty("Content-Type", contentType == null ? "application/x-www-form-urlencoded" : contentType);
            OutputStream o = c.getOutputStream();
            o.write(body.getBytes(StandardCharsets.UTF_8));
            o.close();
        } else if (!method.equals("GET")) {
            c.setDoOutput(true);
            c.setFixedLengthStreamingMode(0);
        }
        int code = c.getResponseCode();
        if (code == 204) return "";
        InputStream in = code >= 400 ? c.getErrorStream() : c.getInputStream();
        if (in == null) return "";
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] buf = new byte[4096];
        int n;
        while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
        in.close();
        if (code >= 400) throw new RuntimeException("http " + code);
        return out.toString("UTF-8");
    }

    private static String token(FrostState st) throws Exception {
        if (!access.isEmpty() && System.currentTimeMillis() < accessUntil - 30_000L) return access;
        String body = "grant_type=refresh_token&refresh_token=" + URLEncoder.encode(st.fx.spotify_refresh, "UTF-8")
            + "&client_id=" + URLEncoder.encode(st.fx.spotify_client_id, "UTF-8");
        JsonObject o = new JsonParser().parse(http("POST", "https://accounts.spotify.com/api/token", body, null, null)).getAsJsonObject();
        access = o.get("access_token").getAsString();
        accessUntil = System.currentTimeMillis() + o.get("expires_in").getAsLong() * 1000L;
        if (o.has("refresh_token")) st.fx.spotify_refresh = o.get("refresh_token").getAsString();
        return access;
    }

    private static Track web(FrostState st) throws Exception {
        String tok = token(st);
        String json = http("GET", "https://api.spotify.com/v1/me/player/currently-playing", null, tok, null);
        Track t = new Track();
        t.source = "Spotify";
        if (json.isEmpty()) return t;
        JsonObject o = new JsonParser().parse(json).getAsJsonObject();
        t.playing = o.has("is_playing") && o.get("is_playing").getAsBoolean();
        t.progressMs = o.has("progress_ms") && !o.get("progress_ms").isJsonNull() ? o.get("progress_ms").getAsLong() : 0;
        JsonElement itemEl = o.get("item");
        if (itemEl == null || itemEl.isJsonNull()) return t;
        JsonObject item = itemEl.getAsJsonObject();
        t.title = item.get("name").getAsString();
        t.durationMs = item.get("duration_ms").getAsLong();
        StringBuilder artists = new StringBuilder();
        if (item.has("artists")) {
            for (JsonElement a : item.getAsJsonArray("artists")) {
                if (artists.length() > 0) artists.append(", ");
                artists.append(a.getAsJsonObject().get("name").getAsString());
            }
        }
        t.artist = artists.toString();
        if (item.has("album")) {
            JsonObject album = item.getAsJsonObject("album");
            t.album = album.get("name").getAsString();
            if (album.has("images") && album.getAsJsonArray("images").size() > 0) {
                com.google.gson.JsonArray imgs = album.getAsJsonArray("images");
                String url = imgs.get(imgs.size() - 1).getAsJsonObject().get("url").getAsString();   // smallest
                if (!url.equals(artUrl)) {
                    artUrl = url;
                    artPixels = loadArt(url);
                }
                t.art = artPixels;
            }
        }
        return t;
    }

    private static int[] loadArt(String url) {
        try {
            HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
            c.setConnectTimeout(6000);
            c.setReadTimeout(8000);
            BufferedImage img = ImageIO.read(c.getInputStream());
            if (img == null) return null;
            BufferedImage small = new BufferedImage(16, 16, BufferedImage.TYPE_INT_ARGB);
            java.awt.Graphics2D g = small.createGraphics();
            g.setRenderingHint(java.awt.RenderingHints.KEY_INTERPOLATION, java.awt.RenderingHints.VALUE_INTERPOLATION_BILINEAR);
            g.drawImage(img, 0, 0, 16, 16, null);
            g.dispose();
            int[] px = new int[256];
            small.getRGB(0, 0, 16, 16, px, 0, 16);
            for (int i = 0; i < px.length; i++) px[i] |= 0xFF000000;
            return px;
        } catch (Throwable t) {
            return null;
        }
    }

    // ------------------------------------------------------------------ local players

    private static String exec(long timeoutMs, String... cmd) {
        try {
            Process p = new ProcessBuilder(cmd).redirectErrorStream(true).start();
            BufferedReader r = new BufferedReader(new InputStreamReader(p.getInputStream(), StandardCharsets.UTF_8));
            StringBuilder sb = new StringBuilder();
            String line;
            while ((line = r.readLine()) != null) sb.append(line).append('\n');
            if (!p.waitFor(timeoutMs, TimeUnit.MILLISECONDS)) p.destroyForcibly();
            return sb.toString().trim();
        } catch (Throwable t) {
            return "";
        }
    }

    private static Track local() {
        Track t = new Track();
        if (windows()) {
            String title = exec(4000, "powershell", "-NoProfile", "-Command",
                "(Get-Process Spotify -ErrorAction SilentlyContinue | Where-Object {$_.MainWindowTitle} | Select-Object -First 1).MainWindowTitle");
            if (title.isEmpty()) return t;
            t.source = "Spotify";
            if (title.equals("Spotify") || title.equals("Spotify Free") || title.equals("Spotify Premium")) {
                t.title = "Paused";
                return t;
            }
            int dash = title.indexOf(" - ");
            t.playing = true;
            if (dash > 0) {
                t.artist = title.substring(0, dash);
                t.title = title.substring(dash + 3);
            } else {
                t.title = title;
            }
            return t;
        }
        String out = exec(2500, "playerctl", "metadata", "--format", "{{title}}\t{{artist}}\t{{album}}\t{{mpris:length}}\t{{position}}\t{{status}}");
        if (out.isEmpty() || out.startsWith("No players")) return t;
        String[] p = out.split("\t", -1);
        if (p.length < 6) return t;
        t.title = p[0];
        t.artist = p[1];
        t.album = p[2];
        try {
            t.durationMs = Long.parseLong(p[3].trim()) / 1000L;
            t.progressMs = Long.parseLong(p[4].trim()) / 1000L;
        } catch (NumberFormatException ignored) {
            // some players do not report a position
        }
        t.playing = p[5].trim().equalsIgnoreCase("Playing");
        t.source = "Player";
        return t;
    }

    // ------------------------------------------------------------------ controls

    public static void playPause(final FrostState st) {
        control(st, "play-pause");
    }

    public static void next(final FrostState st) {
        control(st, "next");
    }

    public static void previous(final FrostState st) {
        control(st, "previous");
    }

    private static void control(final FrostState st, final String what) {
        Thread t = new Thread(new Runnable() {
            public void run() {
                try {
                    if (!st.fx.spotify_refresh.isEmpty() && !st.fx.spotify_client_id.isEmpty()) {
                        String tok = token(st);
                        if (what.equals("play-pause")) {
                            boolean playing = track != null && track.playing;
                            http("PUT", "https://api.spotify.com/v1/me/player/" + (playing ? "pause" : "play"), null, tok, null);
                        } else {
                            http("POST", "https://api.spotify.com/v1/me/player/" + what, null, tok, null);
                        }
                    } else if (windows()) {
                        int key = what.equals("play-pause") ? 179 : what.equals("next") ? 176 : 177;
                        exec(4000, "powershell", "-NoProfile", "-Command",
                            "(New-Object -ComObject WScript.Shell).SendKeys([char]" + key + ")");
                    } else {
                        exec(3000, "playerctl", what);
                    }
                } catch (Throwable ignored) {
                    // no player or no network: nothing to control
                }
            }
        }, "frost-spotify-ctl");
        t.setDaemon(true);
        t.start();
    }

    // ------------------------------------------------------------------ login (PKCE)

    /** Starts the browser login; returns the URL (also copied into the notice in case no browser opens). */
    public static String beginLogin(final FrostState st, final Runnable save) throws Exception {
        String id = st.fx.spotify_client_id;
        if (id == null || id.trim().isEmpty()) return null;
        byte[] rnd = new byte[48];
        new SecureRandom().nextBytes(rnd);
        verifier = Base64.getUrlEncoder().withoutPadding().encodeToString(rnd);
        byte[] hash = MessageDigest.getInstance("SHA-256").digest(verifier.getBytes(StandardCharsets.US_ASCII));
        String challenge = Base64.getUrlEncoder().withoutPadding().encodeToString(hash);
        if (loginServer != null) loginServer.stop(0);
        loginServer = com.sun.net.httpserver.HttpServer.create(new InetSocketAddress("127.0.0.1", 8899), 0);
        loginServer.createContext("/callback", new com.sun.net.httpserver.HttpHandler() {
            public void handle(com.sun.net.httpserver.HttpExchange ex) throws java.io.IOException {
                String q = ex.getRequestURI().getQuery();
                String code = null;
                if (q != null) {
                    for (String kv : q.split("&")) if (kv.startsWith("code=")) code = kv.substring(5);
                }
                String msg = "Frost: Spotify connected. You can close this tab.";
                try {
                    if (code == null) throw new RuntimeException("no code");
                    String body = "grant_type=authorization_code&code=" + code + "&redirect_uri=" + URLEncoder.encode(REDIRECT, "UTF-8")
                        + "&client_id=" + URLEncoder.encode(st.fx.spotify_client_id, "UTF-8") + "&code_verifier=" + verifier;
                    JsonObject o = new JsonParser().parse(http("POST", "https://accounts.spotify.com/api/token", body, null, null)).getAsJsonObject();
                    st.fx.spotify_refresh = o.get("refresh_token").getAsString();
                    access = o.get("access_token").getAsString();
                    accessUntil = System.currentTimeMillis() + o.get("expires_in").getAsLong() * 1000L;
                    save.run();
                    st.notify("Spotify connected");
                } catch (Throwable e) {
                    msg = "Frost: Spotify login failed. Check the client id and the redirect URI " + REDIRECT;
                    st.notify("Spotify login failed");
                }
                byte[] b = msg.getBytes(StandardCharsets.UTF_8);
                ex.getResponseHeaders().add("Content-Type", "text/plain; charset=utf-8");
                ex.sendResponseHeaders(200, b.length);
                ex.getResponseBody().write(b);
                ex.close();
                loginServer.stop(1);
                loginServer = null;
            }
        });
        loginServer.start();
        String url = "https://accounts.spotify.com/authorize?response_type=code&client_id=" + URLEncoder.encode(id.trim(), "UTF-8")
            + "&scope=" + URLEncoder.encode("user-read-currently-playing user-read-playback-state user-modify-playback-state", "UTF-8")
            + "&redirect_uri=" + URLEncoder.encode(REDIRECT, "UTF-8") + "&code_challenge_method=S256&code_challenge=" + challenge;
        openBrowser(url);
        return url;
    }

    private static void openBrowser(String url) {
        try {
            if (java.awt.Desktop.isDesktopSupported() && java.awt.Desktop.getDesktop().isSupported(java.awt.Desktop.Action.BROWSE)) {
                java.awt.Desktop.getDesktop().browse(new java.net.URI(url));
                return;
            }
        } catch (Throwable ignored) {
            // fall through to the OS commands
        }
        try {
            if (windows()) new ProcessBuilder("cmd", "/c", "start", "", url).start();
            else new ProcessBuilder("xdg-open", url).start();
        } catch (Throwable ignored) {
            // the URL is also shown in the notice
        }
    }
}
