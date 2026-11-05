package dev.frost.mc;

import dev.frost.core.FrostState;
import dev.frost.core.Menu;
import dev.frost.core.Models;
import dev.frost.core.Radial;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;

/**
 * In-game Frost screens: the menu (HUD editor, trainer, quality; Right Shift) and the radial
 * wheels for skins (R) and capes (X). Logic lives in core/Menu and core/Radial.
 */
public class FrostScreen extends Screen {
    private final Menu menu = new Menu(FrostMc.HOST, FrostState.INSTANCE);
    private final String mode;
    private final int toggleKey;
    private boolean toggleWasDown = true; // the key that opened the wheel is still held
    private Radial radial;

    public FrostScreen() {
        this("menu");
    }

    /** `mode` is "menu", "skins" or "capes". */
    public FrostScreen(String mode) {
        super(Component.literal("Frost"));
        this.mode = mode;
        this.toggleKey = mode.equals("skins") ? GLFW.GLFW_KEY_R : mode.equals("capes") ? GLFW.GLFW_KEY_X : -1;
    }

    private Radial buildRadial() {
        Models.Library lib = FrostState.INSTANCE.library;
        List<Radial.Slot> slots = new ArrayList<>();
        boolean skins = mode.equals("skins");
        for (Models.LibEntry e : skins ? lib.skins : lib.capes) {
            slots.add(new Radial.Slot(e.id, e.name, Radial.parseColor(e.color)));
        }
        String hint = skins ? "Click a skin" : "Click a cape";
        if (slots.isEmpty()) {
            hint = "Start the game from the Frost launcher\nand save skins in the Skin editor";
        }
        return new Radial(skins ? "Skins" : "Capes", hint, slots, skins ? "" : lib.active_cape);
    }

    public void render(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
        Minecraft mc = Minecraft.getInstance();
        final Font font = mc.font;
        final GuiGraphicsExtractor gg = g;
        long window = GLFW.glfwGetCurrentContext();
        boolean down = window != 0L && GLFW.glfwGetMouseButton(window, GLFW.GLFW_MOUSE_BUTTON_LEFT) == GLFW.GLFW_PRESS;
        Menu.Canvas canvas = new Menu.Canvas() {
            public void fill(int x1, int y1, int x2, int y2, int argb) {
                gg.fill(x1, y1, x2, y2, argb);
            }

            public void text(String s, int x, int y, int argb) {
                gg.text(font, s, x, y, argb, false);
            }

            public int width(String s) {
                return font.width(s);
            }

            public int lineHeight() {
                return font.lineHeight;
            }
        };
        // readable text: small GUI scales get blown up (the menu lays itself out in the shrunken space)
        double gs = Math.max(1.0, mc.getWindow().getGuiScale());
        final float u = (float) Math.max(1.0, Math.min(2.0, 3.0 / gs));
        if (toggleKey < 0) {
            menu.ui = u;
            g.pose().pushMatrix();
            g.pose().scale(u, u);
            menu.render(canvas, Math.round(this.width / u), Math.round(this.height / u), Math.round(mouseX / u), Math.round(mouseY / u), down);
            g.pose().popMatrix();
            return;
        }
        // wheels close with their own key as well as Esc
        boolean keyDown = window != 0L && GLFW.glfwGetKey(window, toggleKey) == GLFW.GLFW_PRESS;
        if (keyDown && !toggleWasDown) {
            mc.setScreenAndShow(null);
            return;
        }
        toggleWasDown = keyDown;
        if (radial == null) radial = buildRadial();
        String picked = radial.render(canvas, this.width, this.height, mouseX, mouseY, down);
        if (picked != null) {
            if (mode.equals("skins")) FrostMc.requestSkin(picked);
            else FrostMc.requestCape(picked);
            mc.setScreenAndShow(null);
        }
    }

    public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
        render(g, mouseX, mouseY, delta);
    }

    public boolean isPauseScreen() {
        return false;
    }
}
