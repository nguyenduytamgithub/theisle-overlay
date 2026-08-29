use super::visibility::{
    preset_parameters, LuminanceController, RendererReadback, VisibilityParameters,
    VisibilityPreset, VisibilityRenderer,
};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::fmt;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use windows::core::{factory, Interface, PCSTR};
use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{
    ID3DBlob, D3D_DRIVER_TYPE_HARDWARE, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Buffer, ID3D11DepthStencilView, ID3D11Device, ID3D11DeviceContext,
    ID3D11PixelShader, ID3D11RenderTargetView, ID3D11SamplerState, ID3D11ShaderResourceView,
    ID3D11Texture2D, ID3D11VertexShader, D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_RENDER_TARGET,
    D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SAMPLER_DESC,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DEFAULT,
    D3D11_USAGE_STAGING, D3D11_VIEWPORT,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R32_FLOAT, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIAdapter, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1, DXGI_PRESENT, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetParent, SetWindowPos, HWND_TOP, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, WINDOW_EX_STYLE, WS_CHILD, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_VISIBLE,
};
#[cfg(feature = "devtools")]
use windows::Win32::UI::WindowsAndMessaging::{HWND_TOPMOST, WS_EX_TOPMOST, WS_POPUP};

const VISIBILITY_SHADER: &str = include_str!("visibility.hlsl");
const START_TIMEOUT: Duration = Duration::from_secs(5);
const FRAME_WAIT: Duration = Duration::from_millis(100);
const MAX_INTERVAL_HISTORY: usize = 31;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GpuVisibilityError(String);

impl GpuVisibilityError {
    fn new(context: &str, error: impl fmt::Display) -> Self {
        Self(format!("{context}: {error}"))
    }
}

impl fmt::Display for GpuVisibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GpuVisibilityError {}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GpuSessionConfig {
    pub(crate) host_hwnd: isize,
    pub(crate) output_hwnd: isize,
    pub(crate) game_hwnd: isize,
    pub(crate) source: (i32, i32, i32, i32),
    pub(crate) preset: VisibilityPreset,
    pub(crate) strength: u8,
    pub(crate) force_bright: bool,
}

impl GpuSessionConfig {
    pub(crate) fn new(
        host_hwnd: isize,
        output_hwnd: isize,
        game_hwnd: isize,
        source: (i32, i32, i32, i32),
        preset: VisibilityPreset,
        strength: u8,
        force_bright: bool,
    ) -> Result<Self, String> {
        let (_, _, width, height) = source;
        if host_hwnd == 0
            || output_hwnd == 0
            || game_hwnd == 0
            || host_hwnd == game_hwnd
            || output_hwnd == host_hwnd
            || output_hwnd == game_hwnd
            || width <= 0
            || height <= 0
        {
            return Err(format!(
                "invalid exact-HWND GPU visibility target: host={host_hwnd} output={output_hwnd} game={game_hwnd} source={source:?}"
            ));
        }
        Ok(Self {
            host_hwnd,
            output_hwnd,
            game_hwnd,
            source,
            preset,
            strength: strength.min(100),
            force_bright,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GpuStage {
    Starting,
    Running,
    Recovering,
    Failed,
    Stopped,
}

#[derive(Clone, Debug)]
pub(crate) struct GpuLifecycle {
    stage: GpuStage,
    recovery_used: bool,
    presented_frames: u64,
}

impl GpuLifecycle {
    pub(crate) fn new() -> Self {
        Self {
            stage: GpuStage::Starting,
            recovery_used: false,
            presented_frames: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn stage(&self) -> GpuStage {
        self.stage
    }

    #[cfg(test)]
    pub(crate) fn renderer(&self) -> VisibilityRenderer {
        if self.readback_allowed() {
            VisibilityRenderer::GpuAdaptive
        } else {
            VisibilityRenderer::None
        }
    }

    pub(crate) fn record_present(&mut self) {
        if matches!(self.stage, GpuStage::Starting | GpuStage::Recovering) {
            self.stage = GpuStage::Running;
        }
        if self.stage == GpuStage::Running {
            self.presented_frames = self.presented_frames.saturating_add(1);
        }
    }

    pub(crate) fn record_device_loss(&mut self) -> bool {
        if matches!(self.stage, GpuStage::Stopped | GpuStage::Failed) {
            return false;
        }
        if self.recovery_used {
            self.stage = GpuStage::Failed;
            false
        } else {
            self.recovery_used = true;
            self.stage = GpuStage::Recovering;
            true
        }
    }

    pub(crate) fn record_terminal_failure(&mut self) {
        if self.stage != GpuStage::Stopped {
            self.stage = GpuStage::Failed;
        }
    }

    pub(crate) fn readback_allowed(&self) -> bool {
        self.stage == GpuStage::Running && self.presented_frames > 0
    }

    pub(crate) fn stop(&mut self) -> bool {
        if self.stage == GpuStage::Stopped {
            false
        } else {
            self.stage = GpuStage::Stopped;
            true
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C, align(16))]
pub(crate) struct GpuShaderConstants {
    pub(crate) exposure: f32,
    pub(crate) shadow_lift: f32,
    pub(crate) gamma: f32,
    pub(crate) highlight_knee: f32,
    pub(crate) saturation: f32,
    pub(crate) detail_gain: f32,
    pub(crate) scene_luma: f32,
    pub(crate) force_bright: f32,
    pub(crate) texel_size: [f32; 2],
    padding: [f32; 2],
}

impl GpuShaderConstants {
    pub(crate) fn new(
        parameters: VisibilityParameters,
        texel_size: [f32; 2],
        scene_luma: f32,
        force_bright: bool,
    ) -> Self {
        Self {
            exposure: parameters.exposure,
            shadow_lift: parameters.shadow_lift,
            gamma: parameters.gamma,
            highlight_knee: parameters.highlight_knee,
            saturation: parameters.saturation,
            detail_gain: parameters.detail_gain,
            scene_luma: if scene_luma.is_finite() {
                scene_luma.clamp(0.005, 1.0)
            } else {
                0.18
            },
            force_bright: f32::from(force_bright),
            texel_size,
            padding: [0.0; 2],
        }
    }
}

#[derive(Clone, Debug)]
struct SharedReadback {
    latest: Option<RendererReadback>,
    error: Option<GpuVisibilityError>,
}

impl SharedReadback {
    fn new() -> Self {
        Self {
            latest: None,
            error: None,
        }
    }
}

enum WorkerCommand {
    Stop,
}

/// Owns an exact-window Windows Graphics Capture session and all D3D immediate
/// context work on one worker thread. It never captures a monitor and never
/// copies captured pixels outside the process; only a one-float luminance
/// aggregate is mapped back at four samples per second.
pub(crate) struct GpuVisibilitySession {
    commands: mpsc::Sender<WorkerCommand>,
    shared: Arc<Mutex<SharedReadback>>,
    worker: Option<JoinHandle<()>>,
    output_hwnd: isize,
}

impl GpuVisibilitySession {
    pub(crate) fn start(config: GpuSessionConfig) -> Result<Self, GpuVisibilityError> {
        let (commands, command_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let shared = Arc::new(Mutex::new(SharedReadback::new()));
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("night-vision-gpu".to_string())
            .spawn(move || worker_main(config, command_rx, worker_shared, started_tx))
            .map_err(|error| GpuVisibilityError::new("spawn GPU visibility worker", error))?;

        match started_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                commands,
                shared,
                worker: Some(worker),
                output_hwnd: config.output_hwnd,
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(error) => {
                let _ = commands.send(WorkerCommand::Stop);
                let _ = worker.join();
                Err(GpuVisibilityError::new(
                    "wait for GPU visibility startup",
                    error,
                ))
            }
        }
    }

    pub(crate) fn readback(&self) -> Result<Option<RendererReadback>, GpuVisibilityError> {
        let shared = self
            .shared
            .lock()
            .map_err(|_| GpuVisibilityError("GPU readback lock is poisoned".to_string()))?;
        if let Some(error) = &shared.error {
            return Err(error.clone());
        }
        Ok(shared.latest)
    }

    pub(crate) fn output_hwnd(&self) -> isize {
        self.output_hwnd
    }

    pub(crate) fn stop(mut self) -> Result<(), GpuVisibilityError> {
        self.stop_inner()
    }

    fn stop_inner(&mut self) -> Result<(), GpuVisibilityError> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        let _ = self.commands.send(WorkerCommand::Stop);
        worker
            .join()
            .map_err(|_| GpuVisibilityError("GPU visibility worker panicked".to_string()))
    }
}

impl Drop for GpuVisibilitySession {
    fn drop(&mut self) {
        let _ = self.stop_inner();
    }
}

struct D3dPipeline {
    config: GpuSessionConfig,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain1,
    output: ID3D11RenderTargetView,
    vertex_shader: ID3D11VertexShader,
    visibility_shader: ID3D11PixelShader,
    luma_shader: ID3D11PixelShader,
    sampler: ID3D11SamplerState,
    constants: ID3D11Buffer,
    luma_target: ID3D11Texture2D,
    luma_view: ID3D11RenderTargetView,
    luma_staging: ID3D11Texture2D,
    frame_pool: Direct3D11CaptureFramePool,
    capture_session: GraphicsCaptureSession,
    frame_token: i64,
    frame_rx: mpsc::Receiver<()>,
    luminance: LuminanceController,
    lifecycle: GpuLifecycle,
    intervals: VecDeque<f32>,
    last_presented: Option<Instant>,
    last_luma_sample: Option<Instant>,
}

impl D3dPipeline {
    fn create(config: GpuSessionConfig) -> Result<Self, GpuVisibilityError> {
        unsafe { create_pipeline_for_output(config, HWND(config.output_hwnd as *mut c_void)) }
    }

    fn render_next_frame(&mut self) -> Result<Option<RendererReadback>, GpuVisibilityError> {
        let frame = match self.frame_pool.TryGetNextFrame() {
            Ok(frame) => frame,
            Err(_) => return Ok(None),
        };
        let surface = frame
            .Surface()
            .map_err(|error| GpuVisibilityError::new("read capture frame surface", error))?;
        let access: IDirect3DDxgiInterfaceAccess = surface
            .cast()
            .map_err(|error| GpuVisibilityError::new("open capture surface interface", error))?;
        let texture: ID3D11Texture2D = unsafe { access.GetInterface() }
            .map_err(|error| GpuVisibilityError::new("open D3D11 capture texture", error))?;
        let mut source_view = None;
        unsafe {
            self.device
                .CreateShaderResourceView(&texture, None, Some(&mut source_view))
        }
        .map_err(|error| GpuVisibilityError::new("create frame shader view", error))?;
        let source_view = source_view
            .ok_or_else(|| GpuVisibilityError("D3D11 returned no frame shader view".to_string()))?;

        let now = Instant::now();
        let measured = if luma_sample_due(self.last_luma_sample, now) {
            self.last_luma_sample = Some(now);
            self.measure_scene_luma(&source_view)?
        } else {
            None
        };
        if let Some(measured) = measured {
            let _ = self.luminance.sample_at(measured, now);
        }
        self.render_visibility(&source_view)?;
        unsafe { self.swap_chain.Present(0, DXGI_PRESENT(0)) }
            .ok()
            .map_err(|error| GpuVisibilityError::new("present GPU visibility frame", error))?;

        if let Some(previous) = self.last_presented.replace(now) {
            let interval = now.duration_since(previous).as_secs_f32() * 1000.0;
            if interval.is_finite() && interval > 0.0 {
                if self.intervals.len() == MAX_INTERVAL_HISTORY {
                    self.intervals.pop_front();
                }
                self.intervals.push_back(interval);
            }
        }
        self.lifecycle.record_present();
        let _ = frame.Close();

        if !self.lifecycle.readback_allowed() || self.intervals.is_empty() {
            return Ok(None);
        }
        let mut intervals = self.intervals.iter().copied().collect::<Vec<_>>();
        intervals.sort_by(f32::total_cmp);
        let median_interval_ms = intervals[intervals.len() / 2];
        Ok(Some(RendererReadback {
            renderer: VisibilityRenderer::GpuAdaptive,
            game_hwnd: self.config.game_hwnd,
            source: self.config.source,
            preset: self.config.preset,
            presented_frames: self.lifecycle.presented_frames,
            last_presented_at: now,
            median_interval_ms,
            scene_luma: self.luminance.smoothed(),
        }))
    }

    fn measure_scene_luma(
        &self,
        source_view: &ID3D11ShaderResourceView,
    ) -> Result<Option<f32>, GpuVisibilityError> {
        let viewport = D3D11_VIEWPORT {
            Width: 1.0,
            Height: 1.0,
            MinDepth: 0.0,
            MaxDepth: 1.0,
            ..Default::default()
        };
        unsafe {
            self.context.OMSetRenderTargets(
                Some(&[Some(self.luma_view.clone())]),
                None::<&ID3D11DepthStencilView>,
            );
            self.context.RSSetViewports(Some(&[viewport]));
            self.context.VSSetShader(&self.vertex_shader, None);
            self.context.PSSetShader(&self.luma_shader, None);
            self.context
                .PSSetShaderResources(0, Some(&[Some(source_view.clone())]));
            self.context
                .PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            self.context
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.context.Draw(3, 0);
            self.context.PSSetShaderResources(0, Some(&[None]));
            self.context
                .CopyResource(&self.luma_staging, &self.luma_target);
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(&self.luma_staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .map_err(|error| GpuVisibilityError::new("map GPU luminance aggregate", error))?;
        let measured = if mapped.pData.is_null() {
            None
        } else {
            Some(unsafe { *(mapped.pData.cast::<f32>()) })
        };
        unsafe { self.context.Unmap(&self.luma_staging, 0) };
        Ok(measured
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.005, 1.0)))
    }

    fn render_visibility(
        &self,
        source_view: &ID3D11ShaderResourceView,
    ) -> Result<(), GpuVisibilityError> {
        let (_, _, width, height) = self.config.source;
        let parameters = preset_parameters(
            self.config.preset,
            self.config.strength,
            self.luminance.smoothed(),
        );
        let constants = GpuShaderConstants::new(
            parameters,
            [1.0 / width as f32, 1.0 / height as f32],
            self.luminance.smoothed(),
            self.config.force_bright,
        );
        let viewport = D3D11_VIEWPORT {
            Width: width as f32,
            Height: height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
            ..Default::default()
        };
        unsafe {
            self.context.UpdateSubresource(
                &self.constants,
                0,
                None,
                (&constants as *const GpuShaderConstants).cast::<c_void>(),
                0,
                0,
            );
            self.context.OMSetRenderTargets(
                Some(&[Some(self.output.clone())]),
                None::<&ID3D11DepthStencilView>,
            );
            self.context.RSSetViewports(Some(&[viewport]));
            self.context.VSSetShader(&self.vertex_shader, None);
            self.context.PSSetShader(&self.visibility_shader, None);
            self.context
                .PSSetShaderResources(0, Some(&[Some(source_view.clone())]));
            self.context
                .PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            self.context
                .PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            self.context
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.context.Draw(3, 0);
            self.context.PSSetShaderResources(0, Some(&[None]));
        }
        Ok(())
    }

    fn close(&mut self) {
        self.lifecycle.stop();
        let _ = self.frame_pool.RemoveFrameArrived(self.frame_token);
        let _ = self.capture_session.Close();
        let _ = self.frame_pool.Close();
    }
}

fn worker_main(
    initial: GpuSessionConfig,
    commands: mpsc::Receiver<WorkerCommand>,
    shared: Arc<Mutex<SharedReadback>>,
    started: mpsc::SyncSender<Result<(), GpuVisibilityError>>,
) {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    if !initialized {
        let _ = started.send(Err(GpuVisibilityError(
            "initialize GPU worker COM apartment failed".to_string(),
        )));
        return;
    }

    let mut pipeline = match D3dPipeline::create(initial) {
        Ok(pipeline) => {
            let _ = started.send(Ok(()));
            pipeline
        }
        Err(error) => {
            let _ = started.send(Err(error));
            unsafe { CoUninitialize() };
            return;
        }
    };

    let mut running = true;
    let mut recovery_used = false;
    while running {
        match commands.try_recv() {
            Ok(WorkerCommand::Stop) | Err(mpsc::TryRecvError::Disconnected) => {
                running = false;
                continue;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        if pipeline.frame_rx.recv_timeout(FRAME_WAIT).is_ok() {
            match pipeline.render_next_frame() {
                Ok(Some(readback)) => {
                    if let Ok(mut state) = shared.lock() {
                        state.latest = Some(readback);
                        state.error = None;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    if recovery_used {
                        pipeline.lifecycle.record_terminal_failure();
                        set_worker_error(&shared, error);
                        running = false;
                    } else {
                        recovery_used = true;
                        let _ = pipeline.lifecycle.record_device_loss();
                        let config = pipeline.config;
                        pipeline.close();
                        match D3dPipeline::create(config) {
                            Ok(next) => pipeline = next,
                            Err(recovery_error) => {
                                set_worker_error(
                                    &shared,
                                    GpuVisibilityError::new(
                                        "recover GPU visibility after frame failure",
                                        recovery_error,
                                    ),
                                );
                                running = false;
                            }
                        }
                    }
                }
            }
        }
    }
    pipeline.close();
    unsafe { CoUninitialize() };
}

fn set_worker_error(shared: &Arc<Mutex<SharedReadback>>, error: GpuVisibilityError) {
    if let Ok(mut state) = shared.lock() {
        state.latest = None;
        state.error = Some(error);
    }
}

fn luma_sample_due(last_sample: Option<Instant>, now: Instant) -> bool {
    last_sample.is_none_or(|last| {
        now.checked_duration_since(last)
            .is_some_and(|elapsed| elapsed >= Duration::from_millis(250))
    })
}

#[cfg(feature = "devtools")]
pub(crate) fn run_machine_probe(
    game_hwnd: isize,
    source: (i32, i32, i32, i32),
    preset: VisibilityPreset,
    strength: u8,
    duration: Duration,
) -> Result<RendererReadback, GpuVisibilityError> {
    let host = unsafe { create_probe_host(source) }?;
    let output = match create_output_window(host.0 as isize, source) {
        Ok(output) => output,
        Err(error) => {
            let _ = unsafe { DestroyWindow(host) };
            return Err(error);
        }
    };
    let config = match GpuSessionConfig::new(
        host.0 as isize,
        output,
        game_hwnd,
        source,
        preset,
        strength,
        true,
    ) {
        Ok(config) => config,
        Err(error) => {
            let _ = destroy_output_window(output);
            unsafe { DestroyWindow(host) }.map_err(|cleanup| {
                GpuVisibilityError::new("destroy invalid probe host", cleanup)
            })?;
            return Err(GpuVisibilityError(error));
        }
    };
    let session = match GpuVisibilitySession::start(config) {
        Ok(session) => session,
        Err(error) => {
            let _ = destroy_output_window(output);
            let _ = unsafe { DestroyWindow(host) };
            return Err(error);
        }
    };
    let deadline = Instant::now() + duration;
    let mut latest = None;
    while Instant::now() < deadline {
        if let Some(readback) = session.readback()? {
            latest = Some(readback);
        }
        thread::sleep(Duration::from_millis(20));
    }
    session.stop()?;
    destroy_output_window(output)?;
    unsafe { DestroyWindow(host) }
        .map_err(|error| GpuVisibilityError::new("destroy GPU probe host", error))?;
    latest.ok_or_else(|| {
        GpuVisibilityError("GPU probe completed without a presented frame".to_string())
    })
}

#[cfg(feature = "devtools")]
unsafe fn create_probe_host(source: (i32, i32, i32, i32)) -> Result<HWND, GpuVisibilityError> {
    let (left, top, width, height) = source;
    let host = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0 | WS_EX_TOPMOST.0),
            windows::core::w!("STATIC"),
            windows::core::w!("TheIsle Visibility GPU Probe"),
            WS_POPUP | WS_VISIBLE,
            left,
            top,
            width,
            height,
            None,
            None,
            None,
            None,
        )
    }
    .map_err(|error| GpuVisibilityError::new("create GPU probe host", error))?;
    unsafe {
        SetWindowPos(
            host,
            Some(HWND_TOPMOST),
            left,
            top,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    }
    .map_err(|error| GpuVisibilityError::new("show GPU probe host", error))?;
    Ok(host)
}

pub(crate) fn create_output_window(
    host_hwnd: isize,
    source: (i32, i32, i32, i32),
) -> Result<isize, GpuVisibilityError> {
    let (_, _, width, height) = source;
    let child = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0),
            windows::core::w!("STATIC"),
            windows::core::w!("TheIsle Visibility GPU"),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            width,
            height,
            Some(HWND(host_hwnd as *mut c_void)),
            None,
            None,
            None,
        )
    }
    .map_err(|error| GpuVisibilityError::new("create GPU child window", error))?;
    unsafe {
        SetWindowPos(
            child,
            Some(HWND_TOP),
            0,
            0,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    }
    .map_err(|error| GpuVisibilityError::new("show GPU child window", error))?;
    Ok(child.0 as isize)
}

pub(crate) fn destroy_output_window(output_hwnd: isize) -> Result<(), GpuVisibilityError> {
    if output_hwnd == 0 {
        return Ok(());
    }
    unsafe { DestroyWindow(HWND(output_hwnd as *mut c_void)) }
        .map_err(|error| GpuVisibilityError::new("destroy GPU output window", error))
}

unsafe fn create_pipeline_for_output(
    config: GpuSessionConfig,
    child: HWND,
) -> Result<D3dPipeline, GpuVisibilityError> {
    let actual_parent = unsafe { GetParent(child) }
        .map_err(|error| GpuVisibilityError::new("verify GPU output parent", error))?;
    if actual_parent != HWND(config.host_hwnd as *mut c_void) {
        return Err(GpuVisibilityError(format!(
            "GPU output parent mismatch: expected={} actual={}",
            config.host_hwnd, actual_parent.0 as isize
        )));
    }
    let (_, _, width, height) = config.source;
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }
    .map_err(|error| GpuVisibilityError::new("create D3D11 hardware device", error))?;
    let device =
        device.ok_or_else(|| GpuVisibilityError("D3D11 returned no device".to_string()))?;
    let context =
        context.ok_or_else(|| GpuVisibilityError("D3D11 returned no context".to_string()))?;

    let dxgi_device: IDXGIDevice = device
        .cast()
        .map_err(|error| GpuVisibilityError::new("open DXGI device", error))?;
    let adapter = unsafe { dxgi_device.GetAdapter() }
        .map_err(|error| GpuVisibilityError::new("open DXGI adapter", error))?;
    let dxgi_factory: IDXGIFactory2 = unsafe { adapter.GetParent() }
        .map_err(|error| GpuVisibilityError::new("open DXGI factory", error))?;
    let swap_desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width as u32,
        Height: height as u32,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        ..Default::default()
    };
    let swap_chain =
        unsafe { dxgi_factory.CreateSwapChainForHwnd(&device, child, &swap_desc, None, None) }
            .map_err(|error| GpuVisibilityError::new("create HWND swap chain", error))?;
    let back_buffer: ID3D11Texture2D = unsafe { swap_chain.GetBuffer(0) }
        .map_err(|error| GpuVisibilityError::new("open swap-chain back buffer", error))?;
    let mut output = None;
    unsafe { device.CreateRenderTargetView(&back_buffer, None, Some(&mut output)) }
        .map_err(|error| GpuVisibilityError::new("create output render target", error))?;
    let output =
        output.ok_or_else(|| GpuVisibilityError("D3D11 returned no output target".to_string()))?;

    let vs_code = compile_shader("VSMain", "vs_5_0")?;
    let ps_code = compile_shader("PSMain", "ps_5_0")?;
    let luma_code = compile_shader("LumaPS", "ps_5_0")?;
    let mut vertex_shader = None;
    let mut visibility_shader = None;
    let mut luma_shader = None;
    unsafe { device.CreateVertexShader(&vs_code, None, Some(&mut vertex_shader)) }
        .map_err(|error| GpuVisibilityError::new("create vertex shader", error))?;
    unsafe { device.CreatePixelShader(&ps_code, None, Some(&mut visibility_shader)) }
        .map_err(|error| GpuVisibilityError::new("create visibility shader", error))?;
    unsafe { device.CreatePixelShader(&luma_code, None, Some(&mut luma_shader)) }
        .map_err(|error| GpuVisibilityError::new("create luminance shader", error))?;
    let vertex_shader = vertex_shader
        .ok_or_else(|| GpuVisibilityError("D3D11 returned no vertex shader".to_string()))?;
    let visibility_shader = visibility_shader
        .ok_or_else(|| GpuVisibilityError("D3D11 returned no visibility shader".to_string()))?;
    let luma_shader = luma_shader
        .ok_or_else(|| GpuVisibilityError("D3D11 returned no luminance shader".to_string()))?;

    let sampler_desc = D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
        MaxLOD: f32::MAX,
        ..Default::default()
    };
    let mut sampler = None;
    unsafe { device.CreateSamplerState(&sampler_desc, Some(&mut sampler)) }
        .map_err(|error| GpuVisibilityError::new("create visibility sampler", error))?;
    let sampler =
        sampler.ok_or_else(|| GpuVisibilityError("D3D11 returned no sampler".to_string()))?;

    let buffer_desc = D3D11_BUFFER_DESC {
        ByteWidth: std::mem::size_of::<GpuShaderConstants>() as u32,
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        ..Default::default()
    };
    let mut constants = None;
    unsafe { device.CreateBuffer(&buffer_desc, None, Some(&mut constants)) }
        .map_err(|error| GpuVisibilityError::new("create visibility constant buffer", error))?;
    let constants = constants
        .ok_or_else(|| GpuVisibilityError("D3D11 returned no constant buffer".to_string()))?;

    let luma_default_desc = D3D11_TEXTURE2D_DESC {
        Width: 1,
        Height: 1,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R32_FLOAT,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        ..Default::default()
    };
    let mut luma_target = None;
    unsafe { device.CreateTexture2D(&luma_default_desc, None, Some(&mut luma_target)) }
        .map_err(|error| GpuVisibilityError::new("create GPU luminance target", error))?;
    let luma_target = luma_target
        .ok_or_else(|| GpuVisibilityError("D3D11 returned no luminance target".to_string()))?;
    let mut luma_view = None;
    unsafe { device.CreateRenderTargetView(&luma_target, None, Some(&mut luma_view)) }
        .map_err(|error| GpuVisibilityError::new("create luminance render view", error))?;
    let luma_view = luma_view
        .ok_or_else(|| GpuVisibilityError("D3D11 returned no luminance view".to_string()))?;
    let luma_staging_desc = D3D11_TEXTURE2D_DESC {
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        ..luma_default_desc
    };
    let mut luma_staging = None;
    unsafe { device.CreateTexture2D(&luma_staging_desc, None, Some(&mut luma_staging)) }
        .map_err(|error| GpuVisibilityError::new("create luminance readback texture", error))?;
    let luma_staging = luma_staging.ok_or_else(|| {
        GpuVisibilityError("D3D11 returned no luminance readback texture".to_string())
    })?;

    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }
        .map_err(|error| GpuVisibilityError::new("create WinRT D3D device", error))?;
    let capture_device: IDirect3DDevice = inspectable
        .cast()
        .map_err(|error| GpuVisibilityError::new("open WinRT capture device", error))?;
    let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .map_err(|error| GpuVisibilityError::new("open capture-item factory", error))?;
    let item: GraphicsCaptureItem =
        unsafe { interop.CreateForWindow(HWND(config.game_hwnd as *mut c_void)) }.map_err(
            |error| GpuVisibilityError::new("create capture item for exact game HWND", error),
        )?;
    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &capture_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        SizeInt32 {
            Width: width,
            Height: height,
        },
    )
    .map_err(|error| GpuVisibilityError::new("create free-threaded capture pool", error))?;
    let (frame_tx, frame_rx) = mpsc::sync_channel(1);
    let frame_token = frame_pool
        .FrameArrived(&windows::Foundation::TypedEventHandler::new(move |_, _| {
            let _ = frame_tx.try_send(());
            Ok(())
        }))
        .map_err(|error| GpuVisibilityError::new("subscribe to captured frames", error))?;
    let capture_session = frame_pool
        .CreateCaptureSession(&item)
        .map_err(|error| GpuVisibilityError::new("create exact-window capture session", error))?;
    let _ = capture_session.SetIsCursorCaptureEnabled(false);
    let _ = capture_session.SetIsBorderRequired(false);
    capture_session
        .StartCapture()
        .map_err(|error| GpuVisibilityError::new("start exact-window capture", error))?;

    Ok(D3dPipeline {
        config,
        device,
        context,
        swap_chain,
        output,
        vertex_shader,
        visibility_shader,
        luma_shader,
        sampler,
        constants,
        luma_target,
        luma_view,
        luma_staging,
        frame_pool,
        capture_session,
        frame_token,
        frame_rx,
        luminance: LuminanceController::new(0.18),
        lifecycle: GpuLifecycle::new(),
        intervals: VecDeque::with_capacity(MAX_INTERVAL_HISTORY),
        last_presented: None,
        last_luma_sample: None,
    })
}

fn compile_shader(entry: &str, target: &str) -> Result<Vec<u8>, GpuVisibilityError> {
    let entry = std::ffi::CString::new(entry)
        .map_err(|error| GpuVisibilityError::new("encode shader entry", error))?;
    let target = std::ffi::CString::new(target)
        .map_err(|error| GpuVisibilityError::new("encode shader target", error))?;
    let mut code: Option<ID3DBlob> = None;
    let mut errors: Option<ID3DBlob> = None;
    let result = unsafe {
        D3DCompile(
            VISIBILITY_SHADER.as_ptr().cast::<c_void>(),
            VISIBILITY_SHADER.len(),
            PCSTR::null(),
            None,
            None,
            PCSTR(entry.as_ptr().cast()),
            PCSTR(target.as_ptr().cast()),
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if let Err(error) = result {
        let detail = errors
            .and_then(|blob| unsafe {
                let pointer = blob.GetBufferPointer().cast::<u8>();
                (!pointer.is_null()).then(|| {
                    String::from_utf8_lossy(std::slice::from_raw_parts(
                        pointer,
                        blob.GetBufferSize(),
                    ))
                    .trim()
                    .to_string()
                })
            })
            .unwrap_or_else(|| error.to_string());
        return Err(GpuVisibilityError::new("compile visibility shader", detail));
    }
    let code =
        code.ok_or_else(|| GpuVisibilityError("shader compiler returned no bytecode".to_string()))?;
    let bytes = unsafe {
        std::slice::from_raw_parts(code.GetBufferPointer().cast::<u8>(), code.GetBufferSize())
    };
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::{GpuLifecycle, GpuSessionConfig, GpuShaderConstants, GpuStage, VISIBILITY_SHADER};
    use crate::night_vision::visibility::{
        preset_parameters, VisibilityPreset, VisibilityRenderer,
    };

    #[test]
    fn shader_contract_is_bounded_and_uses_local_contrast() {
        for required in [
            "cbuffer VisibilityConstants",
            "VSMain",
            "PSMain",
            "SV_VertexID",
            "Texture2D<float4>",
            "highlightKnee",
            "detailGain",
            "shadowLift",
            "saturate",
        ] {
            assert!(
                VISIBILITY_SHADER.contains(required),
                "visibility shader is missing {required}"
            );
        }
        assert!(
            VISIBILITY_SHADER.matches("sourceTexture.Sample").count() >= 5,
            "visibility shader must use the center plus four neighboring samples"
        );
        for forbidden in ["RWTexture", "Interlocked", "discard", "while ("] {
            assert!(
                !VISIBILITY_SHADER.contains(forbidden),
                "visibility shader contains unbounded or mutating token {forbidden}"
            );
        }
    }

    #[test]
    fn shader_constants_have_exact_hlsl_layout_and_reference_values() {
        assert_eq!(std::mem::size_of::<GpuShaderConstants>(), 48);
        assert_eq!(std::mem::align_of::<GpuShaderConstants>(), 16);
        let parameters = preset_parameters(VisibilityPreset::Ultra, 85, 0.03);
        let constants =
            GpuShaderConstants::new(parameters, [1.0 / 1920.0, 1.0 / 1080.0], 0.03, true);
        assert_eq!(constants.exposure, parameters.exposure);
        assert_eq!(constants.shadow_lift, parameters.shadow_lift);
        assert_eq!(constants.gamma, parameters.gamma);
        assert_eq!(constants.highlight_knee, parameters.highlight_knee);
        assert_eq!(constants.saturation, parameters.saturation);
        assert_eq!(constants.detail_gain, parameters.detail_gain);
        assert_eq!(constants.scene_luma, 0.03);
        assert_eq!(constants.force_bright, 1.0);
        assert_eq!(constants.texel_size, [1.0 / 1920.0, 1.0 / 1080.0]);
    }

    #[test]
    fn session_configuration_accepts_only_exact_nonzero_game_and_host_targets() {
        let valid = GpuSessionConfig::new(
            200,
            300,
            101,
            (100, 200, 1920, 1080),
            VisibilityPreset::Ultra,
            85,
            true,
        )
        .expect("valid exact-HWND configuration");
        assert_eq!(valid.host_hwnd, 200);
        assert_eq!(valid.output_hwnd, 300);
        assert_eq!(valid.game_hwnd, 101);
        assert_eq!(valid.source, (100, 200, 1920, 1080));
        assert_eq!(valid.strength, 85);

        for rejected in [
            (0, 300, 101, (100, 200, 1920, 1080)),
            (200, 0, 101, (100, 200, 1920, 1080)),
            (200, 300, 0, (100, 200, 1920, 1080)),
            (200, 300, 101, (100, 200, 0, 1080)),
            (200, 300, 101, (100, 200, 1920, -1)),
            (101, 300, 101, (100, 200, 1920, 1080)),
            (200, 200, 101, (100, 200, 1920, 1080)),
            (200, 101, 101, (100, 200, 1920, 1080)),
        ] {
            assert!(GpuSessionConfig::new(
                rejected.0,
                rejected.1,
                rejected.2,
                rejected.3,
                VisibilityPreset::Ultra,
                85,
                true,
            )
            .is_err());
        }
    }

    #[test]
    fn lifecycle_is_fail_closed_allows_one_recovery_and_stops_idempotently() {
        let mut lifecycle = GpuLifecycle::new();
        assert_eq!(lifecycle.stage(), GpuStage::Starting);
        assert!(!lifecycle.readback_allowed());
        lifecycle.record_present();
        assert_eq!(lifecycle.stage(), GpuStage::Running);
        assert!(lifecycle.readback_allowed());
        assert_eq!(lifecycle.renderer(), VisibilityRenderer::GpuAdaptive);

        assert!(lifecycle.record_device_loss());
        assert_eq!(lifecycle.stage(), GpuStage::Recovering);
        assert!(!lifecycle.readback_allowed());
        lifecycle.record_present();
        assert_eq!(lifecycle.stage(), GpuStage::Running);
        assert!(
            !lifecycle.record_device_loss(),
            "only one automatic restart is allowed"
        );
        assert_eq!(lifecycle.stage(), GpuStage::Failed);

        assert!(lifecycle.stop());
        assert_eq!(lifecycle.stage(), GpuStage::Stopped);
        assert!(!lifecycle.stop(), "repeated stop must be idempotent");
        assert!(!lifecycle.readback_allowed());
    }

    #[test]
    fn luminance_sampling_uses_an_independent_four_hz_clock() {
        let start = std::time::Instant::now();
        assert!(super::luma_sample_due(None, start));
        assert!(!super::luma_sample_due(
            Some(start),
            start + std::time::Duration::from_millis(249)
        ));
        assert!(super::luma_sample_due(
            Some(start),
            start + std::time::Duration::from_millis(250)
        ));
    }
}
