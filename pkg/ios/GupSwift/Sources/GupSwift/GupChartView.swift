// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

/// `UIView` subclass that hosts a Gup chart rendered via Metal.
///
/// The view:
/// 1. Uses a `CAMetalLayer` as its backing layer.
/// 2. Attaches to a `GupContext` on first layout.
/// 3. Drives rendering via `CADisplayLink` (VSync-paced, no busy loop).
/// 4. Forwards `UITouch` events into the Gup event pipeline.
///
/// ## Usage (UIKit)
///
/// ```swift
/// let ctx = try GupContext()
/// let chartView = GupChartView(context: ctx)
/// view.addSubview(chartView)
/// ```

#if canImport(UIKit)
import UIKit
import QuartzCore

/// A `UIView` subclass that renders a Gup chart using Metal.
public class GupChartView: UIView {
    // MARK: - Properties

    private let context: GupContext
    private var surface: GupSurface?
    private var displayLink: CADisplayLink?

    // MARK: - Layer type

    override public class var layerClass: AnyClass {
        CAMetalLayer.self
    }

    /// The Metal layer backing this view.
    private var metalLayer: CAMetalLayer {
        // swiftlint:disable:next force_cast
        layer as! CAMetalLayer
    }

    // MARK: - Initialisation

    /// Create a chart view backed by the given GPU context.
    ///
    /// - Parameter context: The Gup GPU context that owns the Metal device.
    public init(context: GupContext) {
        self.context = context
        super.init(frame: .zero)
        commonInit()
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) {
        fatalError("GupChartView does not support Interface Builder.")
    }

    private func commonInit() {
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = true
        isMultipleTouchEnabled = true
    }

    // MARK: - Layout & surface

    override public func layoutSubviews() {
        super.layoutSubviews()

        let scale = window?.screen.scale ?? UIScreen.main.scale
        let drawableSize = CGSize(
            width: bounds.width * scale,
            height: bounds.height * scale
        )
        metalLayer.drawableSize = drawableSize

        let w = UInt32(drawableSize.width)
        let h = UInt32(drawableSize.height)

        if surface == nil {
            attachSurface(width: w, height: h)
        } else {
            surface?.resize(width: w, height: h)
        }
    }

    private func attachSurface(width: UInt32, height: UInt32) {
        let viewPtr = Unmanaged.passUnretained(self).toOpaque()
        var vcPtr: UnsafeMutableRawPointer? = nil
        if let vc = findViewController() {
            vcPtr = Unmanaged.passUnretained(vc).toOpaque()
        }

        let sid = gup_surface_attach_layer(
            context.rawHandle, viewPtr, vcPtr, width, height
        )
        guard sid != 0 else { return }
        surface = GupSurface(context: context, surfaceId: sid)
        startDisplayLink()
    }

    // MARK: - Display link

    private func startDisplayLink() {
        guard displayLink == nil else { return }
        let link = CADisplayLink(target: self, selector: #selector(displayLinkFired(_:)))
        link.add(to: .main, forMode: .common)
        displayLink = link
    }

    @objc private func displayLinkFired(_ link: CADisplayLink) {
        surface?.renderFrame()
    }

    override public func willMove(toWindow newWindow: UIWindow?) {
        super.willMove(toWindow: newWindow)
        if newWindow == nil {
            displayLink?.invalidate()
            displayLink = nil
        }
    }

    override public func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil && displayLink == nil && surface != nil {
            startDisplayLink()
        }
    }

    // MARK: - Touch forwarding

    override public func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesBegan(touches, with: event)
        forwardTouches(touches, phase: 0)
    }

    override public func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesMoved(touches, with: event)
        forwardTouches(touches, phase: 1)
    }

    override public func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesEnded(touches, with: event)
        forwardTouches(touches, phase: 2)
    }

    override public func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        super.touchesCancelled(touches, with: event)
        forwardTouches(touches, phase: 3)
    }

    private func forwardTouches(_ touches: Set<UITouch>, phase: UInt8) {
        let scale = Float(window?.screen.scale ?? UIScreen.main.scale)
        let vw = Float(bounds.width)
        let vh = Float(bounds.height)

        for touch in touches {
            let loc = touch.location(in: self)
            gup_touch_event(
                context.rawHandle,
                UInt64(touch.hash),
                Float(loc.x),
                Float(loc.y),
                phase,
                scale,
                touch.timestamp,
                vw,
                vh
            )
        }
    }

    // MARK: - Helpers

    private func findViewController() -> UIViewController? {
        var responder: UIResponder? = self
        while let next = responder?.next {
            if let vc = next as? UIViewController { return vc }
            responder = next
        }
        return nil
    }
}
#endif
