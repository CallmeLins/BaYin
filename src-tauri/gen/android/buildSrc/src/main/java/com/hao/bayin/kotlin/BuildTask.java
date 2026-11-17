package com.hao.bayin.kotlin;

import org.gradle.api.DefaultTask;
import org.gradle.api.GradleException;
import org.gradle.api.tasks.Input;
import org.gradle.api.tasks.Optional;
import org.gradle.api.tasks.TaskAction;

import java.io.File;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Builds the Rust cdylib for one Android target by invoking cargo directly.
 *
 * Differs from the upstream Tauri template: that one re-enters
 * `npm run -- tauri android android-studio-script` (a nested CLI invocation,
 * slow and hard to debug). Here we call cargo ourselves and inject the NDK
 * clang/ar toolchain so cc-rs based crates (ring, oboe, bundled sqlite) work.
 */
public class BuildTask extends DefaultTask {

    @Input
    private String rootDirRel;

    /** Rust target triple, e.g. aarch64-linux-android */
    @Input
    private String target;

    @Input
    private boolean release;

    @Input
    @Optional
    private List<String> cargoArgs = new ArrayList<>();

    public String getRootDirRel() { return rootDirRel; }
    public void setRootDirRel(String v) { this.rootDirRel = v; }

    public String getTarget() { return target; }
    public void setTarget(String v) { this.target = v; }

    public boolean getRelease() { return release; }
    public void setRelease(boolean v) { this.release = v; }

    public List<String> getCargoArgs() { return cargoArgs; }
    public void setCargoArgs(List<String> v) { this.cargoArgs = v; }

    @TaskAction
    public void assemble() {
        if (rootDirRel == null) throw new GradleException("rootDirRel cannot be null");
        if (target == null) throw new GradleException("target cannot be null");

        File srcTauri = new File(getProject().getProjectDir(), rootDirRel);
        if (!srcTauri.isDirectory()) {
            throw new GradleException("src-tauri dir not found: " + srcTauri.getAbsolutePath());
        }

        List<String> args = new ArrayList<>();
        args.add("build");
        args.add("--target");
        args.add(target);
        args.add("--lib");
        if (release) args.add("--release");
        // Always embed the frontend (custom protocol). The car build never runs a dev
        // server, so the WebView must always load assets baked into the binary.
        args.add("--features");
        args.add("custom-protocol");
        args.addAll(cargoArgs);

        Map<String, String> env = buildNdkEnv(target);

        getProject().getLogger().lifecycle(
                "[rust] cargo " + String.join(" ", args) + " (cwd=" + srcTauri + ")");

        org.gradle.process.ExecResult result = getProject().exec(spec -> {
            spec.setWorkingDir(srcTauri);
            spec.setExecutable("cargo");
            spec.setArgs(args);
            spec.setIgnoreExitValue(true);
            Map<String, Object> full = new HashMap<>();
            full.putAll(System.getenv());
            full.putAll(env);
            spec.setEnvironment(full);
        });

        if (result.getExitValue() != 0) {
            throw new GradleException("cargo build failed, exit=" + result.getExitValue());
        }
    }

    /**
     * cc-rs needs explicit CC/AR/CXX per target; .cargo/config.toml can only
     * provide the linker. Derive the toolchain from NDK_HOME.
     */
    private static Map<String, String> buildNdkEnv(String triple) {
        Map<String, String> env = new HashMap<>();

        String ndkHome = System.getenv("NDK_HOME");
        if (ndkHome == null || ndkHome.isEmpty()) {
            ndkHome = System.getenv("ANDROID_NDK_HOME");
        }
        if (ndkHome == null || ndkHome.isEmpty()) {
            return env;
        }

        String bin = ndkHome + File.separator + "toolchains" + File.separator + "llvm"
                + File.separator + "prebuilt" + File.separator + "windows-x86_64"
                + File.separator + "bin" + File.separator;

        String clangPrefix = clangPrefixFor(triple);
        String key = triple.replace('-', '_');
        String suffix = System.getProperty("os.name", "").toLowerCase().contains("windows") ? ".cmd" : "";
        String cc = bin + clangPrefix + "24-clang" + suffix;
        String cxx = bin + clangPrefix + "24-clang++" + suffix;
        String ar = bin + "llvm-ar" + (suffix.isEmpty() ? "" : ".exe");
        String ranlib = bin + "llvm-ranlib" + (suffix.isEmpty() ? "" : ".exe");

        env.put("CC_" + key, cc);
        env.put("CXX_" + key, cxx);
        env.put("AR_" + key, ar);
        env.put("RANLIB_" + key, ranlib);
        env.put("CC", cc);
        env.put("CXX", cxx);
        env.put("AR", ar);
        env.put("RANLIB", ranlib);
        // cargo 侧 linker：Windows 用 .cmd 包装，Linux/macOS 直接可执行
        env.put("CARGO_TARGET_" + key.toUpperCase() + "_LINKER", cc);

        String path = System.getenv("PATH");
        env.put("PATH", bin + File.pathSeparator + (path == null ? "" : path));
        return env;
    }

    /** Rust triple -> NDK clang prefix (armv7 is special: armv7a) */
    private static String clangPrefixFor(String triple) {
        switch (triple) {
            case "aarch64-linux-android":
                return "aarch64-linux-android";
            case "armv7-linux-androideabi":
                return "armv7a-linux-androideabi";
            case "i686-linux-android":
                return "i686-linux-android";
            case "x86_64-linux-android":
                return "x86_64-linux-android";
            default:
                return triple;
        }
    }
}