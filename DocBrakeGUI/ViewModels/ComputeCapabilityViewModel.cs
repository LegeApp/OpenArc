using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Windows.Input;
using DocBrake.Commands;
using DocBrake.Services;
using System.Windows;

namespace DocBrake.ViewModels
{
    public class ComputeCapabilityViewModel : INotifyPropertyChanged
    {
        private ComputeCapability _selectedCapability = new();
        private bool _isCudaAvailable;
        private bool _isOpenClAvailable;
        private bool _showAllOptions = true;
        private readonly NativeGpuService _gpuService;

        public ObservableCollection<ComputeCapability> AvailableCapabilities { get; private set; } = new();

        public event PropertyChangedEventHandler? PropertyChanged;

        public ComputeCapability SelectedCapability
        {
            get => _selectedCapability;
            set
            {
                if (_selectedCapability != value)
                {
                    _selectedCapability = value;
                    OnPropertyChanged();
                }
            }
        }


        public bool ShowAllOptions
        {
            get => _showAllOptions;
            set
            {
                if (_showAllOptions != value)
                {
                    _showAllOptions = value;
                    OnPropertyChanged();
                }
            }
        }


        public bool IsCudaAvailable
        {
            get => _isCudaAvailable;
            set
            {
                if (_isCudaAvailable != value)
                {
                    _isCudaAvailable = value;
                    OnPropertyChanged();
                }
            }
        }


        public bool IsOpenClAvailable
        {
            get => _isOpenClAvailable;
            set
            {
                if (_isOpenClAvailable != value)
                {
                    _isOpenClAvailable = value;
                    OnPropertyChanged();
                }
            }
        }


        public ICommand ToggleDisplayCommand { get; }

        public ComputeCapabilityViewModel()
        {
            AvailableCapabilities = new ObservableCollection<ComputeCapability>();
            ToggleDisplayCommand = new RelayCommand(_ => ShowAllOptions = !ShowAllOptions);
            
            // Initialize the GPU service
            _gpuService = NativeGpuService.Instance;
            
            // Detect capabilities using the native service
            DetectCapabilities();
        }

        private void DetectCapabilities()
        {
            // Use the NativeGpuService to detect actual hardware capabilities
            AvailableCapabilities.Clear();

            // Detect CUDA
            IsCudaAvailable = _gpuService.HasCuda;
            
            // Detect OpenCL
            IsOpenClAvailable = _gpuService.HasOpenCL;

            // Add all capabilities in order of preference
            AvailableCapabilities.Add(new ComputeCapability { Type = ComputeType.Cuda, IsAvailable = IsCudaAvailable });
            AvailableCapabilities.Add(new ComputeCapability { Type = ComputeType.OpenCL, IsAvailable = IsOpenClAvailable });
            AvailableCapabilities.Add(new ComputeCapability { Type = ComputeType.CPU, IsAvailable = true });

            // Select the best available option based on the active backend
            SelectedCapability = SelectBestCapability();
        }

        private ComputeCapability SelectBestCapability()
        {
            // Get the actual active backend from the native service
            var activeBackend = _gpuService.ActiveBackend;
            
            // Map the native backend enum to our ComputeType
            ComputeType activeType = ComputeType.CPU; // Default to CPU
            
            switch (activeBackend)
            {
                case GpuBackendType.Cuda:
                    activeType = ComputeType.Cuda;
                    break;
                case GpuBackendType.OpenCL:
                    activeType = ComputeType.OpenCL;
                    break;
                default:
                    activeType = ComputeType.CPU;
                    break;
            }
            
            // Find the corresponding capability in our collection
            foreach (var capability in AvailableCapabilities)
            {
                if (capability.Type == activeType && capability.IsAvailable)
                {
                    return capability;
                }
            }

            // If the active backend is not available, fall back to best available
            // Priority: CUDA > OpenCL > CPU
            foreach (var capability in AvailableCapabilities)
            {
                if (capability.IsAvailable)
                {
                    return capability;
                }
            }

            // Default to CPU as fallback
            // Default to CPU as fallback (should be the last one in the list now)
            return AvailableCapabilities[AvailableCapabilities.Count - 1];
        }

        protected virtual void OnPropertyChanged([CallerMemberName] string? propertyName = null)
        {
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName ?? string.Empty));
        }
    }

    public class ComputeCapability
    {
        public ComputeType Type { get; set; }
        public bool IsAvailable { get; set; }

        public string DisplayName => Type.ToString();
    }

    public enum ComputeType
    {
        Cuda,
        OpenCL,
        CPU
    }

}
