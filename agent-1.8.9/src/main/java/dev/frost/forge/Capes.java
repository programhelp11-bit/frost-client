package dev.frost.forge;

import dev.frost.core.FrostState;
import dev.frost.core.Models;
import net.minecraft.client.Minecraft;
import net.minecraft.client.network.NetworkPlayerInfo;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.util.ResourceLocation;
import net.minecraftforge.fml.relauncher.ReflectionHelper;

import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/** Shows the cape a Frost user picked: loads the PNG and sets it as that player's cape texture. */
public final class Capes {
    private static final Map<String, byte[]> BYTES = new ConcurrentHashMap<String, byte[]>();
    private static final Map<String, ResourceLocation> TEX = new ConcurrentHashMap<String, ResourceLocation>();
    private static final Map<String, Long> FAILED = new ConcurrentHashMap<String, Long>();
    private static final Set<String> PENDING = ConcurrentHashMap.newKeySet();
    private static boolean reflectFailed;

    private Capes() {}

    private static String key(Models.CosmeticEntry e) {
        String k = (e.cape_hash != null && !e.cape_hash.isEmpty()) ? e.cape_hash : e.cape_url;
        return k.toLowerCase().replaceAll("[^a-z0-9._-]", "_");
    }

    private static void download(final String k, final String url) {
        Long f = FAILED.get(k);
        if (f != null && System.currentTimeMillis() - f < 60000L) return;
        if (!PENDING.add(k)) return;
        Thread t = new Thread(new Runnable() {
            public void run() {
                try {
                    HttpURLConnection c = (HttpURLConnection) new URL(url).openConnection();
                    c.setConnectTimeout(8000);
                    c.setReadTimeout(15000);
                    if (c.getResponseCode() != 200) throw new RuntimeException("http");
                    InputStream in = c.getInputStream();
                    ByteArrayOutputStream out = new ByteArrayOutputStream();
                    byte[] buf = new byte[8192];
                    int n;
                    while ((n = in.read(buf)) > 0) {
                        out.write(buf, 0, n);
                        if (out.size() > 512 * 1024) throw new RuntimeException("big");
                    }
                    in.close();
                    BYTES.put(k, out.toByteArray());
                } catch (Exception e) {
                    FAILED.put(k, System.currentTimeMillis());
                } finally {
                    PENDING.remove(k);
                }
            }
        }, "frost-cape-dl");
        t.setDaemon(true);
        t.start();
    }

    public static void tick(Minecraft mc, int ticks) {
        if (reflectFailed || mc.getNetHandler() == null || mc.theWorld == null || ticks % 10 != 0) return;
        for (NetworkPlayerInfo info : mc.getNetHandler().getPlayerInfoMap()) {
            String uuid = info.getGameProfile().getId().toString().replace("-", "");
            Models.CosmeticEntry e = FrostState.INSTANCE.cosmetics.get(uuid);
            if (e == null || !e.frost || e.cape_url == null || e.cape_url.isEmpty()) continue;
            String k = key(e);
            ResourceLocation loc = TEX.get(k);
            if (loc == null) {
                byte[] png = BYTES.get(k);
                if (png == null) {
                    download(k, e.cape_url);
                    continue;
                }
                try {
                    BufferedImage img = ImageIO.read(new ByteArrayInputStream(png));
                    if (img == null) throw new RuntimeException("png");
                    loc = new ResourceLocation("frost_client", "cape/" + k);
                    mc.getTextureManager().loadTexture(loc, new DynamicTexture(img));
                    TEX.put(k, loc);
                } catch (Exception ex) {
                    FAILED.put(k, System.currentTimeMillis());
                    BYTES.remove(k);
                    continue;
                }
            }
            if (info.getLocationCape() != loc) {
                try {
                    ReflectionHelper.setPrivateValue(NetworkPlayerInfo.class, info, loc, "locationCape", "field_178862_f");
                } catch (Throwable t) {
                    reflectFailed = true;
                    return;
                }
            }
        }
    }
}
