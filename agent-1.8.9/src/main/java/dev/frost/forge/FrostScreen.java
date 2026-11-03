package dev.frost.forge;

import dev.frost.core.FrostState;
import dev.frost.core.Menu;
import dev.frost.core.Models;
import dev.frost.core.Radial;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.FontRenderer;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.GuiScreen;
import org.lwjgl.input.Keyboard;
import org.lwjgl.input.Mouse;

import java.util.ArrayList;
import java.util.List;

/** In-game Frost screens for 1.8.9: the menu (Right Shift) and the skin (R) / cape (X) wheels. */
public class FrostScreen extends GuiScreen {
    private final Menu menu = new Menu(FrostForge.HOST, FrostState.INSTANCE);
    private final String mode;
    private final int toggleKey;
    private boolean toggleWasDown = true;
    private Radial radial;

    public FrostScreen() {
        this("menu");
    }

    public FrostScreen(String mode) {
        this.mode = mode;
        this.toggleKey = mode.equals("skins") ? Keyboard.KEY_R : mode.equals("capes") ? Keyboard.KEY_X : -1;
    }

    private Radial buildRadial() {
        Models.Library lib = FrostState.INSTANCE.library;
        List<Radial.Slot> slots = new ArrayList<Radial.Slot>();
        boolean skins = mode.equals("skins");
        for (Models.LibEntry e : skins ? lib.skins : lib.capes) {
            slots.add(new Radial.Slot(e.id, e.name, Radial.parseColor(e.color)));
        }
        String hint = skins ? "Click a skin" : "Click a cape";
        if (slots.isEmpty()) hint = "Start the game from the Frost launcher\nand save skins in the Skin editor";
        return new Radial(skins ? "Skins" : "Capes", hint, slots, skins ? "" : lib.active_cape);
    }

    @Override
    public void drawScreen(int mouseX, int mouseY, float partialTicks) {
        final FontRenderer font = Minecraft.getMinecraft().fontRendererObj;
        Menu.Canvas canvas = new Menu.Canvas() {
            public void fill(int x1, int y1, int x2, int y2, int argb) {
                Gui.drawRect(x1, y1, x2, y2, argb);
            }

            public void text(String s, int x, int y, int argb) {
                font.drawStringWithShadow(s, x, y, argb);
            }

            public int width(String s) {
                return font.getStringWidth(s);
            }

            public int lineHeight() {
                return font.FONT_HEIGHT;
            }
        };
        boolean down = Mouse.isButtonDown(0);
        if (toggleKey < 0) {
            menu.render(canvas, this.width, this.height, mouseX, mouseY, down);
            return;
        }
        boolean keyDown = Keyboard.isKeyDown(toggleKey);
        if (keyDown && !toggleWasDown) {
            mc.displayGuiScreen(null);
            return;
        }
        toggleWasDown = keyDown;
        if (radial == null) radial = buildRadial();
        String picked = radial.render(canvas, this.width, this.height, mouseX, mouseY, down);
        if (picked != null) {
            if (mode.equals("skins")) FrostForge.requestSkin(picked);
            else FrostForge.requestCape(picked);
            mc.displayGuiScreen(null);
        }
    }

    @Override
    public boolean doesGuiPauseGame() {
        return false;
    }
}
